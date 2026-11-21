# AB-EXT-006 依赖与分发审计

## 当前基线与开发包

审计基线为 97803b16276903ffe4b6945966f21adeaad11eeb。七份主输入均与交接记录中的 Git blob 一致。当前根目录没有获批项目许可证。两个 npm 根包声明为 UNLICENSED，仓库内 Rust 包按各自清单记录为项目代码。该状态保留待所有者审定。 [D09](../../12-decisions.md) · [开源与分发核验](../../15-validation-gates.md#4-开源与分发)

开发包来自 GitHub Actions 44122458362、macOS Job 134177203395、artifact 14350183579。工件名称、包内 build-record 和 GitHub 合成树共同锁定实际构建提交 df35231eab29aa2d4a043fd9be60888504507716。GitHub run 的 head 是 PR 源提交 c09e5d5dcc01abf3c9221a7e9e1311c636af700c。ZIP SHA-256 为 cbed165a62710d1d302f3ab28cc3e6e12165d270f9c14ff858d6e93ac00b99e0，DMG SHA-256 为 e6373b07b5d76cb1d329b455b6a1dfc41595735be02416b2aeea974e740df03f。包记录标明开发渠道与临时链接器签名。 [构建](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44122458362/job/134177203395) · [开发归档](https://github.com/wanderbot-auto/AgentBaton/actions/runs/44122458362/artifacts/14350183579)

Apple Silicon 上只读挂载观察到 AgentBaton 0.1.0 的三个 ARM64 可执行文件 agentbaton-desktop、baton、batond，另有 Info.plist、icon.icns 和指向 /Applications 的安装快捷链接。包内未发现独立的第三方声明、外部 Provider 或远程伴随可执行文件。otool 仅报告 macOS 系统动态库与框架，例如 WebKit、AppKit 和 libSystem。静态库、内嵌资源与生成代码需要以各自的链接和输入收据确认。图标与基线中的 icon.icns 字节一致，图标源图的权利归属仍随项目代码审定。 [包内观测](artifact/bundle-observations.log)

历史工件五份主输入与本次基线逐字一致。另两份 Rust 锁的第三方名称、版本、来源、校验和一致。根锁和 Tauri 锁分别从 93、442 项增至 94、443 项，变化为内部 agentbaton-remote-host 0.1.0 节点及 agentbaton-core 指向该节点的依赖。保留历史包和当前基线各自的范围。 [Git 树与锁](artifact/git-inputs.json) · [两份锁差异](artifact/cargo-lock-delta.json)

## 依赖范围

| 范围 | 证据与归属 |
| --- | --- |
| 桌面运行 | 通过 macOS ARM64 的 Cargo normal 图标出运行候选。Tauri、Wry、系统框架桥接与前端入口在本范围。React、React DOM、scheduler 及 @tauri-apps/api 由锁和精确发行包支持，单个 Web 模块是否进入最终资源仍需生成的输入清单。 |
| 桌面编译、测试与代码生成 | Tauri build、proc-macro、CLI、Vite、TypeScript、Vitest、jsdom、native-smoke 和跨平台构建可选包按各自证据归类。检查时保留生成代码归属，Vite 默认可注入 modulepreload polyfill。因此有 dev 标记的包也需要按其产物贡献核实。 |
| 本机核心及 CLI | 根锁中的正常依赖分别由 batond 与 baton 的 ARM64 normal 图给出。rusqlite 包装层和 libsqlite3-sys 归属分别核对。当前清单请求 bundled SQLite，历史 batond 中出现 SQLite 格式标记。vendored SQLite C 原件与项目公开说明单独对应。 |
| 自用远程伴随 | macOS ARM64、Linux ARM64、Linux x64 分别保留目标图。单独产品归档、实际依赖及每平台 NOTICE 为候选交付项。历史 DMG 的复制范围见上。 |
| 基础设施 | 根 npm 只有项目本身。环境烟测 Cargo 锁只有独立项目本身。requirements-infra.txt 只固定 PyYAML、jsonschema 两个直接版本，实际传递解析有效期单独记录。requirements-browser.txt 作为补充测试输入保存四项精确轮子摘要。 |
| 系统与工具链 | macOS 的系统框架从宿主加载。Rust 标准库、链接器输出、生成代码、系统 SDK 条款和图标权利还需针对最终发行物做归属取证。历史编译器的源码声明已按构建记录中的 Rust 提交获取。 |

[锁清单与声明](LOCK-INVENTORY.json) · [核验说明](REPRODUCE.md) · [Vite build.modulePreload](https://vite.dev/config/build-options.html#build-modulepreload) · [SQLite 原作者声明](https://www.sqlite.org/copyright.html)

## macOS ARM64 精确 MPL 图

以下版本的校验和在历史 Tauri 锁和当前 Tauri 锁相同。两个提交都以 custom-protocol 单独重放 Cargo normal（去除 proc-macro）及 normal+build 图。历史正式开发包的 Tauri release 构建日志也分别记录了四个版本的 Compiling 行。该日志证明构建输入参与，成品的静态链接归属和生成资源来源仍需要链接清单或可归因符号证据。

| 包与声明 | 当前及历史 ARM64 依赖路径 | 原始声明和缺口 |
| --- | --- | --- |
| cssparser 0.37.0 · MPL-2.0 | 构建或宏路径。桌面 → tauri-build 2.7.4 → tauri-utils 2.10.2 → dom_query 0.28.0 → cssparser。selectors 也依赖 cssparser。normal 图无入口。 | 精确 .crate 的 Cargo.toml 与 LICENSE，记录中保留两者摘要。 |
| dtoa-short 0.3.5 · MPL-2.0 | 构建或宏路径。桌面 → tauri-build 2.7.4 → tauri-utils 2.10.2 → dom_query 0.28.0 → cssparser 0.37.0 → dtoa-short。normal 图无入口。 | 精确 .crate 的 Cargo.toml 与 LICENSE。 |
| selectors 0.38.0 · MPL-2.0 | 构建或宏路径。桌面 → tauri-build 2.7.4 → tauri-utils 2.10.2 → dom_query 0.28.0 → selectors。normal 图无入口。 | 精确 .crate 的清单声明和源码文件头。归档与锁定 Stylo 树均未定位该组件单独的 LICENSE 文件，标准正文取 Mozilla 官方条款。 |
| option-ext 0.2.0 · MPL-2.0 | normal 路径为桌面 → tauri 2.12.5 → dirs 7.0.0 → dirs-sys 0.5.0 → option-ext。构建路径为桌面 → tauri-build 2.7.4 → dirs 7.0.0 → dirs-sys 0.5.0 → option-ext。 | 精确 .crate 的 Cargo.toml 与 LICENSE.txt。现有成品尚缺此包的逐项链接归属。 |

如最终发行物分发 MPL 覆盖代码的可执行形式，需要按 Mozilla 条款提供获得对应源码的途径，保留适用声明并处理修改过的受覆盖文件。编译专用工具与生成产物应按实际随包内容判定。其他依赖保留上游声明中的 OR、AND、WITH 和旧式分隔写法，由审定人确定实际选择及相应义务。 [当前图](graphs/mpl-current.log) · [历史图](graphs/mpl-historical.log) · [实际构建摘录](artifact/release-build-extract.log) · [Mozilla MPL 2.0 第 3 节](https://www.mozilla.org/en-US/MPL/2.0/)

## 发行候选需要补齐的证据

- 对拟分发包生成可核对的 Cargo 链接与 Web 资源输入收据。以文件和版本映射核对静态 Rust 标准库、SQLite、构建注入代码、字体和图标，以及确实使用的上游版权和 NOTICE。
- 将许可及署名说明与二进制一道打包并测试可访问性。Apache-2.0、BSD、MIT、Unicode 及其他许可的实际条件依其原文逐项确认，MPL 对应源码及访问指引由实际分发归属决定。 [ASF 许可第 4 节](https://www.apache.org/licenses/LICENSE-2.0) · [Mozilla 许可第 3 节](https://www.mozilla.org/en-US/MPL/2.0/)
- 审计产物和历史开发包供内部审阅。正式分发范围、受保护签名环境和项目许可证依执行门单独批准。 [所有者审定材料](OWNER-REVIEW.md)
