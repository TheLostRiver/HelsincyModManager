# Mod 安装作用域与存档配置档

存档配置档只管理 Steam 账号、存档目录及备份设置。Mod 安装状态属于游戏安装目录。
切换、重命名或删除存档配置档，不改变已安装 Mod、重定向目标、插件选择、安装备份或恢复记录。

## 数据边界

- `Profile`、`ProfileRepository` 和 `ActiveProfileProvider` 服务存档备份。
- `ModInstallationContext` 表示游戏安装：`game_id`、`installation_id`、`scope_id`。
- `ModInstallationScopeService` 从已配置的游戏目录取得安装身份；界面不提交任意根目录。
- `JsonModInstallationScopeRepository` 以游戏 ID 与规范化目录的摘要登记安装身份。
  Windows 路径大小写和分隔符差异不产生另一份安装；同一 `GameInstance.id` 的不同目录必须区分。

现有 `InstallManifest`、插件选择、重定向选择、恢复记录和派生查询中的 `ProfileId` / `profile_id`
暂作存储兼容字段。它们承载后端登记的安装命名空间，不引用存档配置档表。
这些内部字段不作为让用户切换 Mod 配置的功能。

前端通过 `get_mod_installation_context` 读取后端 `scopeId`，旧安装 DTO 的 `profileId` 填入该值。
`ModInstallationProvider` 只随游戏目录状态更新，游戏目录变化时立即屏蔽旧作用域，迟到响应不能
覆盖新作用域。切换存档账号不清空 Mod 选择、不触发安装状态扫描，也不作废 Mod 卡片摘要。

## 存量兼容

安装登记保存在应用数据的 `install/installation-scopes.json`。首次登记可保留唯一旧记录的
命名空间，包括已不存在的存档配置档编号。清单、备份引用、重定向绑定、恢复 token 和快照保持原样。
可信且完全为空的旧清单不覆盖其他有效记录；未完成或失败状态的空清单仍属于恢复证据。

没有旧记录时，首个目录使用兼容命名空间 `default`；它是后端持久绑定，不是活动存档配置档。
之后的新目录使用独立 `installation-<摘要>` 命名空间。切回已登记的目录会复用其安装记录。
此行为也适用于删除全部存档自建配置档之后。

登记通过受控目录句柄写入临时文件、同步并原子替换。先持久保存
`installation-scopes.next.json`，再更新主文件；副本必须是主文件安装集合的相同或追加版本。
主文件缺失时可从已写入的副本读取原绑定，不能把现有命名空间分配给另一目录。
版本不支持、记录损坏、副本矛盾、目录链接逃逸等情况拒绝继续登记，不删除原数据。

多份尚未归属的旧安装或选择记录不能通过选择当前存档配置档、拼接清单或猜测文件归属来解决。
返回 `mod_installation_legacy_ambiguous`，界面显示记录核对提示；本版不自动合并这些记录。
需要核对原清单、恢复事务和备份后制定受控迁移，不能手工删除记录来消除提示。
移动游戏目录也不会自动把旧目录的安装身份迁移到新路径。

## 写入和删除

Tauri 安装、重定向、选择与恢复入口校验当前安装作用域。实际安装、卸载、重装、外部接管和恢复
任务在写入准入中再次核验，排队期间更换目录后不能拿旧命名空间写新目录。
任务继续使用原有 InstallPlan、manifest、backup、rollback/recovery 及审计链。

进程内游戏写锁与跨进程 `game-profile-write` 锁按游戏串行，不因存档账号或旧命名空间不同而分叉。
锁名和类型名保留兼容；`save-profile-write` 仍区分存档账号，获取顺序仍为 `save -> game`。
Mod 目录或存档目录的扫描、hash、计划生成继续放在游戏写锁外。

删除库条目时从安装登记、清单和恢复目录枚举全部命名空间，不查询存档配置档列表。
任一清单仍有该 Mod 的安装条目，或任一独立安装／重装恢复记录仍引用它，都拒绝回收包内容。
没有配置游戏目录时也可请求库删除预览，由后端统一确认是否安全。

## CLI

安装命令提供 `--installation-scope auto`，默认由后端选择已配置游戏目录对应的命名空间。
未配置游戏目录时不能解析安装归属，`install plan` 返回 `game_instance_unavailable`，不创建默认记录。
显式指定的命名空间必须与当前目录匹配；旧 `--profile` 保留为参数别名，不选择存档账号。
不带游戏参数的显式 `install status --installation-scope <编号>` 可用于只读检查旧记录。
`ReadOnlyInstallAutomation` 只检查登记，不创建目录、锁文件或登记文件。

存档备份命令继续使用 `--profile` 指定存档配置档；这部分含义和持久化行为不变。

验证见[测试指南](TESTING.md#mod-菜单与安装作用域)。
