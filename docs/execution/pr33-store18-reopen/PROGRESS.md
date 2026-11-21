# AB-REL-007 旧版本恢复进度

1. 冻结恢复收据与交付 1026b0df 的 Git blob 逐字节匹配。恢复文件及旧备份具有相同 SHA-256，Store 19 中间库的版本和摘要按原收据固定。
2. 复用 a4e03303 的正常离线锁定构建。验证历史 Git 树、源码归档、244 个构建文件索引、编译清单和两个 arm64 Mach-O 的原始摘要。从冻结原件制作独立工具副本。
3. 在 private HOME、config、data、cache、state 与短运行目录启动旧 Core。旧 CLI 通过实际 Unix IPC 返回 snapshot、projects list、tasks list 和 events 0。四项均退出 0，与恢复 SQLite 的完整相关记录一致。事件按 1 到 5 返回，无缺口。
4. 在同一自有进程组发送 SIGTERM，确认组静止后回收，退出 0。恢复副本的 63 表逻辑状态与整个主文件摘要均未变。随后旧 Core 对 Store 19 对照副本退出 1，返回 VERSION_UNSUPPORTED，未创建 IPC。
5. 八份已有输入的字节和文件身份全程一致。两次运行的进程和 socket 均已消失。验收后删除该演练本身生成的 IPC token，保留有限回执和任务私有原件。
