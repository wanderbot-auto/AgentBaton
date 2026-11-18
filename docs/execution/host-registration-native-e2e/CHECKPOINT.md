# 交付检查点

- 固定产品构建与工作分支 merge base `cde501db847d1bbfc27a298e16cd772afc1a41e9`，源码树 `10f221cd61296df382fdb03ef4c122f035e0aafe`。
- 驱动提交 `522db444e96ccd1b92b3ebf400bd86c133390ba9`，脚本 SHA-256 `6c4d538672fcfdebb596b6d5019ea350607b397ae7fb31e1761ad89bac5e5e6b`。
- 本文件所在提交是证据交付头，完整 SHA 由任务窗口在交接时发送。交付检查从固定 base 及本地跟踪的最新目标头运行。最新目标比固定构建多 Integration 浏览器测试和证据提交，未改变 App 或侧车构建输入。
- 驱动只使用任务工作树内的 release `.app`、已认领的短私有 runtime、启动时取得的直接子进程和只读审计，产品目录无变更。
- 剩余证据为真实 Tauri/WebView 回包丢失和正式安装／升级、签名／公证。远程 Host 联系和执行没有启用。本任务无需产品批准或签名材料。独立验收与集成由 leader 接续。
