# Integration 浏览器 CI schema 19 投影

固定基线 6feaa39ae60ed4094ff9220fc5edb79c895eb627，分支 collab/host19-browser-ci-projector。已核对 AB-START-001、OWNER-001 至 OWNER-005 和独立认领。Leader 维护共享 workflow 与候选清单发布端。

- 修正现行 driver 的报告标签，保留完整旧文件的字节级逆向。
- 成功投影和失败诊断共用严格的 schema 19 输入配对校验。探针绑定实际使用的候选、前端构建记录和私有工具摘要。
- 验证旧格式、错误 schema、跨来源和跨运行输入、工具归属、清理、有限信息及原图的发布门槛。
- 在固定代码提交上构建侧车及前端，运行实际本机两个 Chromium 流程。投影控制、实际本机运行与托管 Linux 分别记录。
