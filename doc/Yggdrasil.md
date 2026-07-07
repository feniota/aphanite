# Yggdrasil

Yggdrasil，北欧神话中的世界树，是 Minecraft 验证服务的名字。在 [Mojang 决定将游戏账户迁移进微软生态](https://minecraft.wiki/w/Java_account_migration)之前，Yggdrasil 是 Minecraft Java 的身份验证服务机制；同时也是现今市面上几乎所有皮肤站的运作方式。

Aphanite 在 `<aphanite_base_url>/api/yggdrasil` 下实现了 Yggdrasil 服务。通过使用 authlib-injector，Aphanite 可以代替 Mojang 服务器执行玩家信息验证和皮肤下发等功能。

## 参考

- [authlib-injector 文档](https://yushijinhun.github.io/authlib-injector/zh/Home.html)
- [Minecraft Wiki - Yggdrasil](https://minecraft.wiki/w/Yggdrasil)
- [Minecraft Wiki - Mojang API](https://minecraft.wiki/w/Mojang_API)
