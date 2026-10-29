/* Generated-fixture-only Linux read interposer. Never loaded into Cargo,
 * rustc, the Host, another daemon, or a Provider. CONTROL_DIR is supplied by
 * the private fixture at compilation. No production program reads these flags.
 * Pause only AFTER a real64KiB read from the exact held dataset dev/inode/size.
 */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <time.h>
#include <stdatomic.h>
#include <pthread.h>
#include <stdio.h>
#include <errno.h>
#include <stdint.h>
#ifndef CONTROL_DIR
#error private generated control directory required
#endif
static ssize_t (*next_read)(int, void *, size_t);
static pthread_once_t once = PTHREAD_ONCE_INIT;
static atomic_int claimed = 0;
static void initialize(void) { next_read = dlsym(RTLD_NEXT, "read"); }
static int private_file(int fd) {
    struct stat s;
    return fstat(fd,&s)==0 && S_ISREG(s.st_mode) && s.st_nlink==1 &&
        s.st_uid==geteuid() && (s.st_mode & 07777)==0600;
}
static int selected(int fd) {
    struct stat actual;
    if(fstat(fd,&actual)!=0 || !S_ISREG(actual.st_mode) || actual.st_nlink!=1 ||
       actual.st_uid!=geteuid() || (actual.st_mode & 07777)!=0600) return 0;
    int target=open(CONTROL_DIR "/identity",O_RDONLY|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC);
    if(target<0) return 0;
    char bytes[160];
    ssize_t n=private_file(target)?next_read(target,bytes,sizeof(bytes)-1):-1;
    close(target);
    if(n<=0) return 0;
    bytes[n]=0;
    unsigned long long device,inode,size;
    char extra;
    if(sscanf(bytes,"%llu %llu %llu %c",&device,&inode,&size,&extra)!=3) return 0;
    return (uint64_t)actual.st_dev==device && (uint64_t)actual.st_ino==inode &&
        (uint64_t)actual.st_size==size;
}
static void marker(const char *name,const char *value) {
    int fd=open(name,O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC,0600);
    if(fd>=0) {size_t n=0;while(value[n])n++;(void)write(fd,value,n);close(fd);}
}
ssize_t read(int fd,void *buffer,size_t count) {
    pthread_once(&once,initialize);
    if(!next_read) {errno=EIO;return -1;}
    ssize_t n=next_read(fd,buffer,count);
    if(n!=65536 || access(CONTROL_DIR "/enabled",F_OK)!=0 || !selected(fd)) return n;
    int expected=0;
    if(!atomic_compare_exchange_strong(&claimed,&expected,1)) return n;
    struct stat s;fstat(fd,&s);
    char observation[256];
    snprintf(observation,sizeof(observation),"{\"pid\":%ld,\"tid\":%ld,\"device\":%llu,\"inode\":%llu,\"read_bytes\":65536}\n",
        (long)getpid(),(long)syscall(SYS_gettid),(unsigned long long)s.st_dev,(unsigned long long)s.st_ino);
    marker(CONTROL_DIR "/entered",observation);
    struct timespec start,now,pause={0,1000000};
    clock_gettime(CLOCK_MONOTONIC,&start);
    while(access(CONTROL_DIR "/release",F_OK)!=0) {
        clock_gettime(CLOCK_MONOTONIC,&now);
        /* Test failure cleanup bound only; not a production deadline change. */
        if(now.tv_sec-start.tv_sec>=10) {
            marker(CONTROL_DIR "/fixture_timeout","bounded test pause exhausted\n");
            break;
        }
        nanosleep(&pause,0);
    }
    marker(CONTROL_DIR "/left","real read returned to its caller\n");
    return n;
}
