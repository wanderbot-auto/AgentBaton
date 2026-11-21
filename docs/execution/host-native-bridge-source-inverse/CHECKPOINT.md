# Host 桌面接线的来源校验交接

窗口 B 任务分支 `collab/host-native-bridge-source-inverse`，候选 target `origin/collab/host-native-bridge-candidate`，基线 `8aa4483d7028f9e9cb65a709bf2659f5e6715efd`，验证源码 `119d3b20dcf2c9ff24bd6fb0b6641447ccd3eae9`。全量范围仅三个约定的 Python 文件和本证据目录。

组合 `lib.rs` 先按整 SHA-256 绑定原始字节，再仅按唯一上下文移除八行 Host 接线。还原后的字节与 Git `1f756ce201b8cf10f4b1798053674f4e960ac77d` 的前驱 blob 相等，然后沿用原后台整文件逆变换和 token pin。旧命令与 `ensure_daemon` 的独立摘要仍校验当前文件；19 条原 Rust 测试源断言与固定值保留。两种完整输入及前驱 Git 对照见 `logs/git-byte-proof.log`。

组合原始失败日志和同名基线复现均已归档。修复后定向 17 项、C3 源检查 3 项通过。全 infra discovery 927 项中 911 通过、16 项既有跳过、0 失败。验证限既有 venv 上的 Python 源检查，C3 的整 crate fmt/Clippy 旧路径失败仍见 C3 记录。Owned 进程已退出。scope、diff 和固定候选合并预检通过，交 leader 独审证据与最终组合。
