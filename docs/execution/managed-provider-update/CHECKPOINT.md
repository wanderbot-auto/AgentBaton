# AB-PRO-011 交接检查点

| 项目 | 固定引用 |
|---|---|
| 认领分支 | `collab/managed-provider-update` |
| 认领基线 | `fb8bab79b091c203be4ec09c110a5a1cfc75de8e` |
| 已测实现 | `f2585a98c75ab8ab59c2b300b0a91d7be96b9bc2` |
| 预检集成目标 | `collab/host-registration-integration` @ `6c207575c60e55a6e421df7f043daf9941593698` |
| 接续 | `leader-control` 集成窗口独立审阅、测试合成提交并决定合并 |

`agentbaton_providers::managed_update::admit_managed_update` 对可信调用者带来的输入执行准入检查。成功时返回原具名连接的下一修订和新任务绑定。两个安装都须位于批准的规范托管根内。程序摘要现场校验，兼容记录须绑定本次安装、版本、Host、连接及运行配置。

登记调用方须从可信记录确定任务状态、已获批准的等效策略、验证 Host 与证据来源。在登记时原子复核当前修订与有效批准，并保留旧修订和程序文件。派发在执行前复核固定摘要。安装记录、进程和配置现阶段没有登记或写入操作。

Apple Silicon 上 12 项准入测试以及格式、编译和严格 Clippy 通过。默认并发在基线和实现上均遇到原安装探针失败，隔离串行复跑 96 项通过。真实 `--version` 和现场 SHA-256 使用本地合成可执行文件，兼容能力由合成记录提供。生产 Provider、下载、更新注册、发布及实际派发未运行。

核对命令、失败日志、编译器版本与锁文件摘要见 [EVIDENCE.json](EVIDENCE.json)。范围与冲突预检见 [scope-preview.txt](logs/scope-preview.txt)。
