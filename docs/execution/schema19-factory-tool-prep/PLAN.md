# Schema 19 · Factory 测试工具准备

认领基准 `375a054e69d3d411f32266c41b03ba2f6c682460`。按 AB-START-001 执行，已核对 OWNER-001 / OWNER-002。负责现行准备器、其设施测试和此目录。共享 workflow 和业务测试另行集成。

- 保留真实同源基准的原红。保持两条 required workflow 所调用的 prepare / verify CLI、普通预构建二进制、时限和退出约束。
- 当前路径要求 API 1、SQLite 19、RPC 与只读库一致、空表、零序列和受管正常关停。历史 schema 18 的现有断言在显式历史输入路径运行。
- 针对文件换位、工具错配、版本错配和失败关停补控制。用 Apple Silicon 和隔离 Linux 实际构建普通同源工具，运行正常准备与校验，归档精确证据。
