# Fresh schema 18 工作流验收

固定基线 44021e1e6e6503a23dc640923cbf45b4e506a289。实测提交 277cccb02c48df23123847c3363a5fb43ef56f49，tree 392431c09fe133f85b3a6759166f70e47cca2f55。

`Product / Rust` 与 `Product / macos-15` 均在原 schema16 门后执行同一显式 fresh18 门。通过固定 HTTPS 地址浅取旧 SHA，校验旧树和旧锁，使用现行锁准备 87 项同源公共 crate，然后按原 feature/target 离线执行。

Apple Silicon 与本地 Linux ARM64 均从独立深度 1 的候选执行原 YAML 命令，真实迁移与恢复退出 0。Rust 测试分别耗时 102.28 与 101.66 秒。旧对象在初始浅克隆中缺失。获取完成后，候选 head、tree 和 `FETCH_HEAD` 与初始值一致。

两端实际错误候选 SHA 均在 checkout 阶段退出 1。另以受控 Cargo 子进程返回 101，步骤保留 101，固定诊断标记只出现在私有迁移日志。公开日志和 Actions step summary 只提供经过界定的八个字段，原有 artifact 路径维持完整字节。

完整基础设施运行 1,021 项，跳过 16 项。新逆向逐字恢复接线前工作流，原 schema19 与历代位置、超时、权限、检查名和发布断言继续通过。

对目标 84540f293a362c1098c6d6309df9cb1e12cb0dc8 的无冲突模拟 tree 为 85e43ef5d363d0a8ac9bfb915e1af79e05cfee5b。普通 Core、Cargo、旧生成器与 fresh18 工作流入口已在实测源码范围内验证。组合目标新增独立 Factory Rust 测试及其证据。Linux 复跑使用本地原生 ARM64 容器与既有 Rust 1.91.1。GitHub 托管作业（含 Ubuntu x64）留待集成验收。

[执行证据](EVIDENCE.json) 记录了原 CLI 日志和收据的绝对路径、摘要、验证的 Git 对象以及各阶段原退出码。
