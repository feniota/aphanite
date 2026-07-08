# Aphanite General API

Aphanite 系统中 Yggdrasil 和 Phenocryst 两者共享的部分称为 General。

该文件定义了 Aphanite General API 的端点定义和实现细节。

本文使用 TypeScript 类型描述 API 期待的请求体/返回的请求体。

## 数据模型

这里定义一些通用的数据模型，可能会在下面的返回值中被引用。

```typescript
// 用户的元信息
type User = {
  id: string; // 用户的 UUID
  name: string; // 用户的名称，注意该字段非唯一
  email: string; // 用户的邮箱
  permissions: Permission[]; // 用户的权限
};

// 用户的权限。在内部是用数字的比特位存储的，但是序列化时会转换成枚举数组，对于客户端来说只需要把这个枚举解析出来就可以了。
const enum Permission {
  Management = "management",
}

// 玩家角色的元数据
type Profile = {
  id: string; // 该玩家角色的 UUID；
  name: string; // 该玩家角色的游戏内名称。**只能为 ASCII 字符串**
  owner: string; // 该玩家角色所属的 Aphanite 用户的 UUID；
};

// 玩家皮肤的数据
type ProfileSkin = {
  skin?: string; // 皮肤的 URL
  model?: "default" | "slim"; // 手臂粗细
  cape?: string; // 披风的 URL
}
```

## 基本约定

若非特殊说明，Aphanite General API 和 Phenocryst API 都应遵循下面的约定。

1. 请求和返回的报文均应为 JSON 格式，并正确包含 `Content-Type: application/json` 头部。
2. 无论请求是否成功，服务器都以下面的格式响应：
    ```typescript
    type Response<Payload> = {
      success: boolean; // 该操作是否成功
      payload?: Payload; // 若操作成功，服务器响应的实际数据。
      reason?: string; // 若操作失败，人类可读的错误原因。
    };

    // 或者，更具体地
    type Response<Payload> = {
      success: true;
      payload: Payload;
    } | {
      success: false;
      reason: string;
    };
    ```
    若请求发生错误，应该正确指定 HTTP 状态码，但 `reason` 的内容应是引发错误的真实原因，不一定要和 Reason Phrase 契合。`payload` 的类型不做限制，可以是任何 JSON 可以表达的类型（具体由业务 API 而定），但不能为空。如果实在没有什么返回的可以使用 `204 No Content`。下面所说的所有回复体类型都视为这里名为 `Payload` 的泛型参数的内容。
3. 下面提及的端点路径是 `<aphanite_base_url>/api` 下的子目录。

## 鉴权

Aphanite 使用基于令牌的鉴权机制。为了避免维护多个系统带来的复杂性，该令牌的颁发使用 Yggdrasil 的格式，和 Yggdrasil 服务共享同一个池。也就是，Phenocryst 客户端只需要登录一次，就可以同时请求 Phenocryst API 和 Yggdrasil API。

然而，虽然令牌和 Minecraft 验证服务共用同一个池，但由于 Aphanite 系统有独特的用户模型，针对 Aphanite 设计的客户端需要请求 Aphanite 自有的 API，以获取属于 Aphanite 内部的用户信息。

下面详细介绍 Aphanite 的鉴权机制。

### 登录

使用用户的邮箱和密码获取用户信息，并颁发一组验证令牌。

```http
POST /auth/login
```

请求体：

```typescript
type Request={
  email: string; // 账户邮箱
  password?: string; // 账户密码（明文）
  totp?: number; // TOTP
}
```

其中 TOTP 应为按照 RFC 6238 计算的六位数，时间步长为 30s。考虑到减轻用户操作的复杂性，应由 Phanerite 而不是用户自行使用生成器计算。

此处引入 TOTP 的意义在于，防止“一密通”用户由于 Aphanite 泄露密码。但是和传统的安全实践不同， Aphanite 系统中的 TOTP 是 1FA，在使用上和密码具有相同的权威性——这也导致 Aphanite 没那么安全就是了。

客户端应该总是使用 TOTP 来获取 Token，将密码当作二级的、备选的方案。

返回体：

```typescript
type Payload={
  access_token: string;
  client_token: string;
  user: User;
}
```

注意，Client Token 仅在 Yggdrasil API 的特殊情形中使用。我们约定，在 Aphanite General 和 Phenocryst 中忽略它的存在。不过，启动器仍然应该存储它，以备不时之需。


### 刷新令牌

令牌的过期时间是 15 日，在有效期内都可以用来鉴权。但是原则上，Phanerite 应该每天刷新一次令牌。由于 Yggdrasil 的令牌刷新 API 涉及 Profile 选择等，这里设计一个更简单的自有 API。

**需要鉴权**。（见下文）

```http
POST /auth/refresh
```

请求体为空。

返回体：

```typescript
type Payload={
  access_token: string; // 新的 Access Token
  user: User; // 令牌对应的用户的信息；注意，启动器应该将这里返回的信息填入本地存储——如果用户有修改自己的信息就能派上用场
}
```

Client Token 在刷新令牌后保持不变；服务端就不再返回了。

### 检查令牌状态

**需要鉴权**

```http
GET /auth/validate
```

如果令牌有效，则返回 `204 No Content`。

### API 鉴权

在 Phenocryst 和其他 General API 中，鉴权通过将 Access Token 作为 Bearer token 放进 `Authorization` 头实现。也就是

```http
POST /api/endpoint HTTP/1.1
Authorization: Bearer access_token
Content-Type: application/json; charset=utf-8
Content-Length: 18

{"request":"body"}
```

如果到需要鉴权的端点的请求不包含鉴权信息，或者鉴权信息错误（如 token 已过期），则返回 401 Unauthorized。

## 用户系统

### 查询用户信息

**需要鉴权**

```http
GET /users/{id}
GET /user
```

其中 id 是目标用户的 UUID。

返回体

```typescript
type Payload=User;
```

权限鉴定：
- 如果不指定 ID（不带 `s` 的端点），或者 ID 是当前用户的 ID，正常返回；
- 如果指定 ID（带 `s` 的端点），且 ID 不是当前用户，则检查当前用户是否具有 Management 权限；有则正常返回。
- 否则，返回 403 Forbidden。
- 如果请求不包含鉴权信息或鉴权错误，返回 401 Unauthorized。

### 修改用户元信息

**需要鉴权**

```http
PATCH /users/{id}
PATCH /user
```

请求体

```typescript
type Request={
  name?:string;
  email?:string;
}
```

返回体

```typescript
type Payload=User;
```

权限鉴定：同[查询用户信息](#查询用户信息)。

注意，Aphanite 的 Email **全局唯一**。如果要更改的邮箱已经被其他用户关联了，则返回 409 Conflict。

### 修改用户密码

```http
PATCH /users/{id}/credentials/password
PATCH /user/credentials/password
```

请求体

```typescript
type Request = {
  old_password?: string;
  totp?: string;
  new_password: string;
}
```

如果成功，返回 `204 No Content`。

权限鉴定：

- 如果未携带有效鉴权信息:
  - 不能请求省略 ID 的端点。
  - 应指定 `totp` 或 `old_password`。
- 如果携带了有效鉴权信息
  - 可以省略 `totp` 和 `old_password`。
  - 其他限制和[查询用户信息](#查询用户信息)相同。

### 颁发或旋转用户的 TOTP 私钥

**需要鉴权**

注意，考虑到用户的实际情况，我们不期望由用户自己管理 TOTP——这是 Phanerite 需要做的事情。

```http
POST /user/credentials/totp
```

请求体为空。

返回体

```typescript
type Payload = {
  private_key: string;
}
```

请求成功后，原有的 TOTP 密钥立即失效，Phanerite 需要将新密钥存储下来。

注意，该端点不提供指定用户 ID 的版本，只在当前已登录的用户上生效。

### 创建用户

**需要鉴权**

```http
POST /user
```

请求体

```typescript
type Request={
  email:string;
  name?:string; // 未指定则使用邮箱
  password:string;
  permissions: Permission[];
}
```

返回体

```typescript
type Payload=User;
```

> [!CAUTION]
>
> 这一个端点应仅供测试使用，因为它让管理员直接拿到新用户的密码，或者反过来让新用户拿到管理员的凭据，具有严重的安全缺陷。**在正式上线之前应去除此端点。**

仅管理员用户可以请求，否则返回 403 Forbidden。

## Profile 系统

Profile 是指 Minecraft 中，一个玩家实体对应的属性，包含玩家名、玩家 UUID 和玩家纹理等信息。鉴于本项目为 Minecraft 设计，Profile 系统也是核心之一，于是将下面的定制 API 归到 General 中。

### 创建 Profile

**需要鉴权**。

```http
POST /profile
```

请求体：

```typescript
type Request = {
  name: string;
}
```

返回体：

```typescript
type Payload = Profile;
```

### 删除 Profile

**需要鉴权**

```http
DELETE /profiles/{id}
```

返回体：

```typescript
type Payload = Profile;
```

- 如果目标 Profile 不存在，返回 404 Not Found。
- 如果目标 Profile 存在，但不属于当前用户：
  - 若当前用户有 Management 权限，成功。
  - 否则，返回 403 Forbidden。


### 获取 Profile 信息

```http
GET /profiles/{id}?with_skin=boolean
```

- `with_skin`: 布尔值；是否在返回中提供皮肤数据。

返回体

```typescript
type Payload={
  metadata: Profile;
  skin?: ProfileSkin;
}
```

注意，Profile 不一定设置了皮肤，也不一定设置了披风：`skin` 的三个字段有可能均为空。

该端点不需要鉴权。

### 修改 Profile 元信息

```http
PATCH /profiles/{id}
```

请求体：

```typescript
type Request={
  name?:string
}
```

返回体：

```typescript
type Payload = Profile;
```

该端点的鉴权和“删除 Profile”端点相同。

### 修改皮肤

使用 [Authlib-injector API](https://yushijinhun.github.io/authlib-injector/zh/Yggdrasil-%E6%9C%8D%E5%8A%A1%E7%AB%AF%E6%8A%80%E6%9C%AF%E8%A7%84%E8%8C%83.html#%E6%9D%90%E8%B4%A8%E4%B8%8A%E4%BC%A0) 实现。

Aphanite 中所有用户的所有玩家都可以上传这两种材质，没有限制。
