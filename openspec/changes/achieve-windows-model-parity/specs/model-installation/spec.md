## Purpose

为六个新模型建立 Windows 固定来源、逐文件摘要、空间预算与离线安装规则。

## ADDED Requirements

### Requirement: 新模型的 Windows 安装清单与空间预算

每个新模型 SHALL 提供独立 Windows 安装清单：固定来源（ModelScope/HF/官方 release）、逐文件 bytes 与 SHA-256、许可证归档、运行时展开与下载缓存预算；安装前 SHALL 复核数据与缓存卷空间，不足时在下载前失败且不激活、不删除已有数据。

#### Scenario: 空缓存安装
- **WHEN** 用户在新隔离根点击安装某新模型
- **THEN** 安装器按固定源逐文件校验并激活；空间不足或摘要不符时不激活且保留已有数据

#### Scenario: 缓存复用
- **WHEN** 另一型号已缓存相同摘要组件
- **THEN** 仅复用摘要一致的组件，不复制其他型号的 Ready 记录或版本事实
