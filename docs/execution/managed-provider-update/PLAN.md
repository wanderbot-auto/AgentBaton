# AB-PRO-011 托管安装更新准入

认领基线 `fb8bab79b091c203be4ec09c110a5a1cfc75de8e`，分支 `collab/managed-provider-update`。依据 D14、AB-PRO-011、AB-START-001 和集成窗口任务合同。

1. 复用 `InstallationRecord`、`NamedConnection`、`SupportManifest` 与 `ExecutionBinding`。核对调用者独立解析的批准策略和候选兼容证据，校验版本探针与文件摘要。
2. 为新任务准备同一连接的下一修订和新安装绑定。保护现有或结果不明的任务，保持配置与身份引用，限制到已批准托管目录及不同安装身份。
3. 用私有合成程序运行真实版本探针，覆盖批准、缺失证据、范围错配、过期证据、外部程序与磁盘变化。执行锁定依赖的 crate 回归、格式、检查和严格 Clippy。
4. 记录版本、固定提交、结果和冲突预检。由集成窗口独立验收。登记、下载、自动切换与生产 Provider 验证由后续接线推进。
