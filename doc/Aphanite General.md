# Aphanite General API

Aphanite 系统中 Yggdrasil 和 Phenocryst 两者共享的部分称为 General。

该文件定义了 Aphanite General API 的端点定义和实现细节。

若非特殊说明，下面使用 TypeScript 类型描述 API 期待的请求体/返回的请求体。请求和返回的报文均应为 JSON 格式，并正确包含 `Content-Type: application/json` 头部。

## 鉴权

Aphanite 使用基于令牌的鉴权机制。为了避免维护多个系统带来的复杂性，该令牌的颁发使用 Yggdrasil 的格式，和 Yggdrasil 服务共享同一个池。也就是，Phenocryst 客户端只需要登录一次，就可以同时请求 Phenocryst API 和 Yggdrasil API。

### 登录

```
POST /api/yggdrasil/authserver/authenticate
```

请求体：

```typescript
{
  username: string; // 账户邮箱
  password: string; // 账户密码（明文）
  clientToken?: string; // 该客户端生成的字符串，用于让服务器区分不同客户端。通常无需指定。
  requestUser: boolean; // 是否应在回复的报文中返回用户的信息
  agent: { // 客户端信息
    name: string, // 客户端名称
    version: number, // 一般来说为 1 就好
  }
}
```

返回体：

```typescript
{
  accessToken: string; // 实际使用的令牌
  clientToken: string;
  selectedProfile?: Profile;
  user?: User;
}
```

注意，在大部分 Yggdrasil API 中，如果指定了 client token，则它会被验证。不过，为了降低复杂度，我们约定，在 Aphanite General 和 Phenocryst 中，Aphanite 和客户端应忽略 client token 的存在。

### 刷新令牌

如果客户端愿意可以请求 Yggdrasil 的令牌刷新 API（`POST /api/yggdrasil/authserver/refresh`），但由于 Aphanite 没有实现令牌的暂时失效状态，因此令牌刷新 API 可能不适合使用。

如果客户端明确只适用于 Aphanite，应该重复请求登录 API，以达到刷新令牌的效果。

### 检查令牌状态

```
POST /authserver/validate
```

请求体：

```typescript
{
  accessToken:string;
  clientToken?:string;
}
```

如果令牌有效，则返回 `204 No Content`。

### 鉴权

在 Phenocryst 和其他 General API 中，鉴权通过将 Access Token 作为 Bearer token 放进 `Authorization` 头实现。也就是

```http
POST /api/endpoint HTTP/1.1
Authorization: Bearer <access token>
Content-Type: application/json; charset=utf-8
Content-Length: 19

{"request":"body"}
```
