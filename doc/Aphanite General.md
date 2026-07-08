# Aphanite General API

Aphanite 系统中 Yggdrasil 和 Phenocryst 两者共享的部分称为 General。

该文件定义了 Aphanite General API 的端点定义和实现细节。

本文使用 TypeScript 类型描述 API 期待的请求体/返回的请求体。

## 数据模型

这里定义一些通用的数据模型，

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
  password: string; // 账户密码（明文）
}
```

返回体：

```typescript
type Payload={
  access_token: string;
  client_token: string;
  user: User;
}
```

注意，在大部分 Yggdrasil API 中，如果指定了 client token，则它会被验证。不过，为了降低复杂度，我们约定，在 Aphanite General 和 Phenocryst 中，Aphanite 和客户端应忽略 client token 的存在。然而，客户端应该存储该 client token，以备 Yggdrasil 相关操作使用。

### 刷新令牌

客户端可以请求 Yggdrasil 的令牌刷新 API（`POST /api/yggdrasil/authserver/refresh`），但由于 Aphanite 没有实现令牌的暂时失效状态，因此令牌刷新 API 可能不适合使用。

如果客户端是专为 Aphanite 设计的，应该重复请求登录 API，以达到刷新令牌的效果。

### 检查令牌状态

```http
POST /api/yggdrasil/authserver/validate
```

请求体：

```typescript
type Request={
  accessToken:string;
  clientToken?:string;
}
```

如果令牌有效，则返回 `204 No Content`。

### 鉴权

在 Phenocryst 和其他 General API 中，鉴权通过将 Access Token 作为 Bearer token 放进 `Authorization` 头实现。也就是

```http
POST /api/endpoint HTTP/1.1
Authorization: Bearer access_token
Content-Type: application/json; charset=utf-8
Content-Length: 18

{"request":"body"}
```
