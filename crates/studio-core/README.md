# studio-core

平台无关的基础契约及桌面能力查询。该 crate 不启动进程、不读取图片、不访问模型目录，也不提供推理、调度或持久化实现。

- `domain`：不可混传的对象 ID、SHA-256 内容身份、与身份分离的位置描述、Job/Attempt 及执行令牌。
- `contracts`：UserRequest、ExecutionPlan、RunManifest 的独立 v1 文档及校验。
- `application` / `ports`：WorkspaceService 通过 WorkspaceQuery 返回真实的“尚未配置推理”状态。

文档边界使用 `from_json()` 和 `to_json()`。这些入口检查版本、必填字段、未知字段和语义约束；直接使用 Serde 或手工构造的对象仍须显式调用 `validate()`。计划与请求的联合约束使用 `validate_against()`；运行记录自动检查其内嵌请求、计划及输入输出事实。

请求中的能力描述只验证数据是否自洽。真正执行前仍需后续 adapter 根据可信目录验证能力、组件和设备，不能把可通过类型校验的数据当成获准执行的计划。

所有测试数据均标为契约 fixture，示例来源使用 `fixture.invalid`。通过契约测试不表示任何模型、运行时、GPU 或真实生成通过验证；SHA-256 字段合法也不等于已读取并计算相应文件的散列。
