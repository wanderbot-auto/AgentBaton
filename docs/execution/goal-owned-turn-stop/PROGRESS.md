# 真实进展

已实现：绑定原session/attempt/intent/run/plan/node/native及已提交turn ID/key的私有停止凭据；相关completed后停止原owned process；事件队列独立的terminal proof；joined未结算留存；原流水固定checkpoint。

45ca：真实12通过，22.12秒；但独审发现同轮update+completed遇满64队列时会丢proof，该12项未覆盖，保留此失败事实。

396：先私存相关proof、保留backpressure失败和当前unknown、不升candidate；新增第13项与exact历史/replay拒绝。首编译因SQLite u64 ToSql失败，exit101，未运行任何测试。

8367：显式checked i64转换，overflow拒绝且不改原授权revision。真实13通过、0失败，24.24秒（wrapper48.142秒）；源与冻结工具未变。独立源码57检查SOURCE_READY。

原Goal hint测试改用合法Summary维持原Actor-hint安全断言，并只换私有send_goal调用；全部旧测试体/时限保留。8367真实原hint12通过14.37秒（wrapper40.320秒）、原sessions2通过0.07秒（wrapper23.883秒）；同源三组共27个互不重名测试通过，不把旧45ca重复计入。完整回归、strictClippy/infra、真实IPC、双Mac/新准确CI尚未验证。原Root003 Intel真实超4秒失败记录不删除，不宣称本修复已通过Mac性能门槛。
