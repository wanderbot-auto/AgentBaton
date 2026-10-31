#!/usr/bin/env python3
"""Create one fresh private copied-interpreter venv; never chmod a shared Python."""
import argparse
import hashlib
import os
from pathlib import Path
import stat
import sys
import venv

MAX_INTERPRETER = 268435456
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


def require(value):
    if not value:
        raise ValueError('Private browser Python preparation refused')


def directory_pin(value):
    return value.st_dev, value.st_ino, value.st_mode, value.st_uid


def file_pin(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_uid, value.st_nlink,
            value.st_size, value.st_mtime_ns, value.st_ctime_ns)


def owned_directory(fd, private=False):
    value = os.fstat(fd)
    require(stat.S_ISDIR(value.st_mode) and value.st_uid == os.geteuid()
            and not value.st_mode & 0o022)
    if private:
        require(stat.S_IMODE(value.st_mode) == 0o700)
    return directory_pin(value)


def image(fd, owned=False):
    before = os.fstat(fd)
    require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= MAX_INTERPRETER)
    if owned:
        require(before.st_uid == os.geteuid() and before.st_nlink == 1)
    else:
        require(before.st_uid in (0, os.geteuid()))
    os.lseek(fd, 0, os.SEEK_SET)
    digest, size = hashlib.sha256(), 0
    while block := os.read(fd, min(1048576, before.st_size - size + 1)):
        size += len(block)
        require(size <= before.st_size)
        digest.update(block)
    require(size == before.st_size and file_pin(os.fstat(fd)) == file_pin(before))
    return before, digest.hexdigest()


def prepare(output):
    """Only a fresh descendant of the owned cwd; failed creations stay for review."""
    require(os.name == 'posix' and sys.version_info.major == 3)
    cwd = Path.cwd()
    path = Path(output)
    parts = path.relative_to(cwd).parts if path.is_absolute() else path.parts
    require(0 < len(parts) <= 8 and all(part not in ('.', '..') and len(os.fsencode(part)) <= 255 for part in parts))
    destination = cwd.joinpath(*parts)
    expected = {'python', 'python3', 'python3.' + str(sys.version_info.minor)}
    source = Path(sys._base_executable).resolve(strict=True)
    require(source.name in expected)
    descriptors, edges, copies = [], [], []
    old_umask = os.umask(0o077)
    try:
        source_fd = os.open(source, FILE_FLAGS); descriptors.append(source_fd)
        source_stamp, source_hash = image(source_fd)
        root_fd = os.open('.', DIR_FLAGS); descriptors.append(root_fd)
        root_pin = owned_directory(root_fd)
        parent = root_fd
        for name in parts[:-1]:
            child = os.open(name, DIR_FLAGS, dir_fd=parent); descriptors.append(child)
            edges.append((parent, name, child, owned_directory(child)))
            parent = child
        os.mkdir(parts[-1], 0o700, dir_fd=parent)
        env_fd = os.open(parts[-1], DIR_FLAGS, dir_fd=parent); descriptors.append(env_fd)
        edges.append((parent, parts[-1], env_fd, owned_directory(env_fd, private=True)))

        def recheck():
            require(directory_pin(os.stat(cwd, follow_symlinks=False)) == root_pin
                    and directory_pin(os.fstat(root_fd)) == root_pin)
            for parent_fd, name, held_fd, pin in edges:
                require(directory_pin(os.stat(name, dir_fd=parent_fd, follow_symlinks=False)) == pin
                        and directory_pin(os.fstat(held_fd)) == pin)
            require(file_pin(os.fstat(source_fd)) == file_pin(source_stamp)
                    and file_pin(os.stat(source, follow_symlinks=False)) == file_pin(source_stamp))

        recheck()
        # Standard venv retains pyvenv.cfg/base-stdlib layout. Never copy HOME,
        # credentials or configuration trees, and never use interpreter symlinks.
        venv.EnvBuilder(symlinks=False, with_pip=True).create(str(destination))
        recheck()
        bin_fd = os.open('bin', DIR_FLAGS, dir_fd=env_fd); descriptors.append(bin_fd)
        edges.append((env_fd, 'bin', bin_fd, owned_directory(bin_fd)))
        require({name for name in os.listdir(bin_fd) if name.startswith('python')} == expected)
        for name in sorted(expected):
            fd = os.open(name, FILE_FLAGS, dir_fd=bin_fd); descriptors.append(fd)
            before, digest = image(fd, owned=True)
            require(digest == source_hash and (before.st_dev, before.st_ino)
                    != (source_stamp.st_dev, source_stamp.st_ino))
            require(file_pin(os.stat(name, dir_fd=bin_fd, follow_symlinks=False)) == file_pin(before))
            copies.append([name, fd, before])
        require(len({(item[2].st_dev, item[2].st_ino) for item in copies}) == len(expected))
        recheck()
        # Validate every copy before any chmod, and revalidate all names/held
        # identities before each mutation. fchmod never follows a pathname.
        for name, fd, before in copies:
            recheck()
            for other_name, other_fd, other_stamp in copies:
                require(file_pin(os.fstat(other_fd)) == file_pin(other_stamp)
                        and file_pin(os.stat(other_name, dir_fd=bin_fd, follow_symlinks=False)) == file_pin(other_stamp))
            os.fchmod(fd, 0o500)
            after, digest = image(fd, owned=True)
            require((after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
                    == (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
                    and stat.S_IMODE(after.st_mode) == 0o500 and digest == source_hash)
            for item in copies:
                if item[1] == fd:
                    item[2] = after
        recheck()
        require({name for name in os.listdir(bin_fd) if name.startswith('python')} == expected)
        for name, fd, final in copies:
            require(file_pin(os.stat(name, dir_fd=bin_fd, follow_symlinks=False)) == file_pin(final)
                    and file_pin(os.fstat(fd)) == file_pin(final))
        return {'prepared_copied_interpreters': len(copies), 'mode': '0500',
                'shared_source_unchanged': True}
    finally:
        for fd in reversed(descriptors):
            os.close(fd)
        os.umask(old_umask)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        prepare(args.output)
    except (ValueError, OSError):
        print('Private browser Python preparation refused', file=sys.stderr)
        return 1
    print('Prepared private browser Python copies; shared source unchanged')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
