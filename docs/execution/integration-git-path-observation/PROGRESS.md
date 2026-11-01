# Progress · Integration Git path observation

生产源码0793完成同次三路径观察，6个纯逆向检查通过；这些不计入Rust产品测试。原代码/所有原测试体可逆向恢复。首次新Python guard生成串有语法错误，加载失败、零测试体，窄修后6PASS；该历史未删除。

主树 `f0e642674e7f2ab887eb15ac6e418b43c0d756db` 编译0793时12个E0616，exit101、零Rust测试、wrapper1.704311937s。3b仅把新测试的6个布尔读取改为原公开getter，6个计数读取用已有Serialize的严格u64只读投影；没有扩大生产字段可见性或删除断言。

主树 `18c0365e248ea294a0eff5792f57812a0937ab2e` 实际 Workspaces --lib45PASS（含新增10+原35，不能再叠加），测试35.67s/wrapper38.399629203s；Integration IPC33PASS，测试101.84s/wrapper125.602976819s。每次 source/tracked/frozen tools 未变。该Root与3b只差先前child-plan-local-activation元数据、STATUS和manifest，运行源码一致。准确命令、tree、INPUTS/RESULT/log哈希见EVIDENCE。

独审SOURCE_READY附Root45实际证据；本次metadata待独审。未跑新Mac/CI，没有测得Mac耗时改善，不声称原db281 ARM21/12失败已修复。原2s/5s、TOCTOU、cancel/unknown、真实IO计费和全部130需求状态保持。

同一Root18c另有严格Clippy（Workspaces+Core/all-targets/-D warnings）exit0，wrapper18.667750135s；完整基础设施793项PASS，测试19.105s/wrapper19.455937190s。793已含本单元6纯guard，不再相加。末尾failed_or_interrupted为有意负向fixture输出，实际外层unittest793 OK；不是总体失败，也不代表产品或Mac验证。新增两个准确命令的INPUTS/RESULT/log哈希已登记。
