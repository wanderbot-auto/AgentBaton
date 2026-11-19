# 检查点

- 状态 `ready`，供 leader 独立审查。
- 基线 `53da4ce20fdfedae4a745e704394072ad6b742ae`。产品与测试提交 `7ba8387594e2fc5a4d03b930e75c1c23bbb7d98c`。第二轮原生复跑在干净执行提交 `e9f7d51be270e40c8f15ae587f1ae4c0f05f76eb`。交付提交与所有证据哈希在本地交接回执。
- 写入范围是两阶段打包入口、其准入测试、本目录。生产路径以原编译 SHA-256 审查 staging 后，验证两个助手，再只签 App 外层。递归严格通过后冻结出货 SHA-256 和资源封印。只读挂载再次核对两类 pin 并递归严格校验。保留现有四文件格式和内容审计，build record 新增签名前后哈希及封印哈希。
- Apple Silicon macOS 26.6.1 (25G76) 和系统 Bash 3.2。实际产品打包成功，独立 provenance 消费成功，两条原生拒绝路径成功。只用固定 `d70362ad1fc4b080537163be60cb830849ee4167` 的同次已构建字节，私有 checkout 仅置换入口。原件输出和回执前后哈希相同。所有自有进程已回收，无自有挂载。
- 私有对照 DMG SHA-256 `e184d9a6eb5ac3da623601840139d015d581cf610f730dc99e8d279f08fcd9c6`。ZIP SHA-256 `16281e7b8c9a4ab4b9393fbeb694313c5d665d898c75d88c15f22884d5e7b926`。本地实物位置见交接回执。压缩报告已脱敏，可以用 `gzip -dc native-02.json.gz` 复核。
- 首次原生复跑的诊断断言过严，产品已拒绝篡改，重跑确认。首轮既有审计的三个超时已保留，基线和候选复跑均通过。未执行重编译、正式签名、公证、Gatekeeper、安装、启动、托管 CI 或发布。

从本工作树复跑，需要可读的原编译文件和原交付目录。

```bash
bash scripts/collab.sh run --native -- bash -eu -c '
  out="$PWD/artifacts/development-adhoc-package-admission"
  mkdir -p "$out"
  python3 docs/execution/development-adhoc-package-admission/replay.py \
    --deliverables ../../deliverables/pr33/d70362ad --output "$out/replay.json"
'
```

对照 DMG 和 ZIP 随每次新建而有独立哈希。以每次的本地回执校验产物。
