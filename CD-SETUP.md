# Aphanite CD 配置指南

本文档说明如何配置 Aphanite 的持续部署（CD）系统。

## 目录

1. [GPG 签名密钥](#gpg-签名密钥)
2. [Infisical Cloud 集成](#infisical-cloud-集成)
3. [GitHub 仓库配置](#github-仓库配置)
4. [工作流程说明](#工作流程说明)

---

## GPG 签名密钥

### 已生成的密钥

主密钥已生成：

- **指纹**: `03E4 5910 85AA 699B 79F5  AA8F E205 AF93 3EE2 C3BF`
- **身份**: `Feniota <trust@feniota.org>`
- **有效期**: 180 天（至 2027-04-23）
- **能力**:
  - 主密钥 [C] (Certification)
  - 子密钥 [S] (Signing)

密钥文件已导出到：

- 公钥: `/tmp/aphanite-signing-public.asc`
- 私钥: `/tmp/aphanite-signing-private.asc`

### 密钥认证（Certification）流程

要让其他密钥为你的签名密钥做认证，需要以下步骤：

#### 1. 导出认证请求

```bash
# 导出公钥
gpg --armor --export trust@feniota.org > aphanite-signing-key.asc

# 导出密钥指纹和 UID
gpg --fingerprint trust@feniota.org
```

#### 2. 发送给认证者

将 `aphanite-signing-key.asc` 发送给需要为你签名的密钥持有者。

#### 3. 认证者执行签名

认证者需要执行：

```bash
# 导入你的公钥
gpg --import aphanite-signing-key.asc

# 验证指纹（确保是正确的密钥）
gpg --fingerprint trust@feniota.org

# 签名（使用自己的密钥）
gpg --sign-key trust@feniota.org

# 导出签名后的公钥
gpg --armor --export trust@feniota.org > aphanite-signing-key-certified.asc
```

#### 4. 收集所有认证

重复步骤 2-3，直到收集到所有需要的认证签名。

#### 5. 导入认证后的公钥

```bash
# 导入带有认证的公钥
gpg --import aphanite-signing-key-certified.asc

# 验证认证
gpg --check-sigs trust@feniota.org
```

#### 6. 发布到密钥服务器（可选）

```bash
# 发布到密钥服务器
gpg --keyserver keyserver.ubuntu.com --send-keys 03E4591085AA699B79F5AA8FE205AF933EE2C3BF
```

### 上传到网站

将以下文件上传到你的网站：

- `aphanite-signing-key.asc`（公钥）
- 密钥指纹信息
- 使用说明

示例 HTML：

```html
<h2>Aphanite Release Signing Key</h2>
<p>Fingerprint: <code>03E4 5910 85AA 699B 79F5 AA8F E205 AF93 3EE2 C3BF</code></p>
<p>Identity: Feniota &lt;trust@feniota.org&gt;</p>
<p>Valid until: 2027-04-23</p>
<p><a href="/aphanite-signing-key.asc">Download Public Key</a></p>

<h3>Verification Instructions</h3>
<pre><code># Import the key
gpg --import aphanite-signing-key.asc

# Verify signature
gpg --verify aphanite-x86_64-unknown-linux-gnu.zip.sig aphanite-x86_64-unknown-linux-gnu.zip
</code></pre>
```

### 密钥认证（Certification）

已生成的签名密钥需要由其他受信任的密钥进行认证。以下是认证命令：

**使用 Forgejo 密钥认证：**

```bash
gpg --default-key 23B26E2B70B23337 --sign-key E205AF933EE2C3BF
```

**使用本地密钥认证：**

```bash
gpg --default-key F0E5924669B17B3B --sign-key E205AF933EE2C3BF
```

**验证认证结果：**

```bash
gpg --check-sigs trust@feniota.org
```

**导出带有认证的公钥：**

```bash
gpg --armor --export trust@feniota.org > aphanite-signing-key-certified.asc
```

---

## Infisical Cloud 集成

### 1. 安装 Infisical CLI

```bash
# Linux
curl -sSfL https://raw.githubusercontent.com/Infisical/infisical-cli/main/install.sh | bash

# 或使用包管理器
brew install infisical  # macOS
```

### 2. 登录 Infisical

```bash
infisical login
```

### 3. 创建项目和环境

```bash
# 创建项目（如果还没有）
infisical projects create aphanite

# 切换到项目
infisical projects switch aphanite

# 创建环境（如果需要，默认项目已自带 prod）
infisical environments create prod
```

### 4. 存储 GPG 私钥

```bash
# 将私钥存储到 Infisical
infisical secrets set GPG_PRIVATE_KEY="$(cat /tmp/aphanite-signing-private-final.asc)" --env prod

# 设置密码（如果有的话，当前生成的密钥没有密码）
infisical secrets set GPG_PASSPHRASE="" --env prod
```

### 5. 配置 GitHub Actions OIDC 集成

#### 在 Infisical 中创建 OIDC 身份

进入 Infisical Dashboard → Project Settings → Identity & Access → Identities → Create Identity：

- 名称：`github-actions-release`
- 认证方式：选择 `GitHub Auth`
- 配置：
  - Repository: `feniota/aphanite`
  - Environment: `prod`
  - Workflow Ref: `refs/tags/v*`（限制只在 tag 推送时可用）

#### 将身份关联到密钥

在 Infisical 中，将上面创建的身份绑定到存放 `GPG_PRIVATE_KEY` 和 `GPG_PASSPHRASE` 的 environment。

#### 在 GitHub 添加 Secret

在 **Settings > Secrets and variables > Actions > Secrets** 中添加：

- `INFISICAL_IDENTITY_ID`: Infisical 机器身份 ID（在 Infisical Dashboard → Access Control → Machine Identities 中查看）
- `INFISICAL_PROJECT_SLUG`: Infisical 项目 slug（在 Infisical Dashboard → Project Settings → General 中查看）

这两个值是标识符而非凭据，但仍以 Secret 形式保存（工作流中通过 `secrets.` 引用）。

#### 备选方案：直接存储到 GitHub Secrets

如果不想使用 Infisical，可以直接将私钥存储到 GitHub Secrets：

```bash
# 导出私钥
gpg --armor --export-secret-keys trust@feniota.org
```

在 GitHub 仓库的 Settings > Secrets and variables > Actions 中添加：

- `GPG_PRIVATE_KEY`: 私钥内容
- `GPG_PASSPHRASE`: 密钥密码（如果有的话）

### 6. 工作流中的 Infisical 配置

工作流已配置为使用 OIDC 认证：

```yaml
- name: Fetch secrets from Infisical
  uses: Infisical/secrets-action@v1.0.16
  with:
    method: "oidc"
    identity-id: ${{ secrets.INFISICAL_IDENTITY_ID }}
    project-slug: ${{ secrets.INFISICAL_PROJECT_SLUG }}
    env-slug: "prod"

- name: Import GPG key
  uses: crazy-max/ghaction-import-gpg@v6
  with:
    gpg_private_key: ${{ env.GPG_PRIVATE_KEY }}
    passphrase: ${{ env.GPG_PASSPHRASE }}
```

---

## GitHub 仓库配置

### 1. Secrets（必需）

在 GitHub 仓库的 **Settings > Secrets and variables > Actions** 中添加以下 secrets：

#### Forgejo 集成

- `FORGEJO_TOKEN`: Forgejo API 令牌（需要 `repo` 权限）

#### Infisical

- `INFISICAL_IDENTITY_ID`: Infisical 机器身份 ID（在 Infisical Dashboard → Access Control → Machine Identities 中查看）
- `INFISICAL_PROJECT_SLUG`: Infisical 项目 slug（在 Infisical Dashboard → Project Settings → General 中查看）

> **注意**：`GPG_PRIVATE_KEY`、`GPG_PASSPHRASE`、`R2_ACCESS_KEY_ID`、`R2_SECRET_ACCESS_KEY`、`R2_ACCOUNT_ID`、`QUAY_USERNAME`、`QUAY_PASSWORD` 都存放在 Infisical 中，通过 OIDC 自动获取，不需要添加到 GitHub Secrets。

### 2. Variables（必需）

在 GitHub 仓库的 **Settings > Secrets and variables > Actions > Variables** 中添加：

#### Forgejo 配置

- `FORGEJO_BASE_URL`: Forgejo 实例 URL（例如 `https://git.feniota.org`）
- `FORGEJO_OWNER`: Forgejo 仓库所有者（例如 `feniota`）
- `FORGEJO_REPO`: Forgejo 仓库名称（例如 `aphanite`）

#### Cloudflare R2 配置

- `R2_BUCKET`: R2 存储桶名称
- `CDN_BASE_URL`: CDN 公开访问 URL（例如 `https://artifacts.feniota.org`）

### 3. 创建 Secrets 和 Variables

#### Forgejo Token

1. 登录 Forgejo
2. 进入 **Settings > Applications**
3. 创建新的访问令牌，勾选 `repo` 权限
4. 复制令牌到 GitHub Secrets

#### Quay.io 凭证

1. 登录 Quay.io
2. 进入 **Account Settings**
3. 生成加密密码或使用 OAuth 访问令牌
4. 保存到 Infisical（`QUAY_USERNAME` / `QUAY_PASSWORD`）

#### Cloudflare R2 凭证

1. 登录 Cloudflare Dashboard
2. 进入 **R2 Object Storage**
3. 创建 API 令牌（具有读写权限）
4. 复制 Access Key ID 和 Secret Access Key
5. 在 **Account Settings** 中找到 Account ID
6. 保存到 Infisical（`R2_ACCESS_KEY_ID` / `R2_SECRET_ACCESS_KEY` / `R2_ACCOUNT_ID`）

---

## 工作流程说明

### 触发条件

工作流在推送匹配 `v*` 模式的标签时触发，例如：

- `v0.1.0`
- `v1.0.0`
- `v2.0.0-beta.1`

### 作业流程

```
┌─────────────────────────────────────────────────────────────┐
│                      Release Workflow                        │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌───────────────┐    ┌──────────────────┐    ┌──────────────┐
│  Build (GNU)  │    │  Build (musl)    │    │ Build (Win)  │
└───────┬───────┘    └────────┬─────────┘    └──────┬───────┘
        │                     │                     │
        └─────────────────────┼─────────────────────┘
                              │
                              ▼
                    ┌──────────────────┐
                    │  Generate        │
                    │  Changelog       │
                    └────────┬─────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐    ┌──────────────┐    ┌─────────────────┐
│   Release     │    │  Upload S3   │    │ Build Container │
└───────────────┘    └──────────────┘    └─────────────────┘
        │
        ▼
┌───────────────────┐
│ Sync to Forgejo   │
└───────────────────┘
```

### 1. Build 作业

为三个平台构建二进制文件：

- `x86_64-unknown-linux-gnu` (Linux glibc)
- `x86_64-unknown-linux-musl` (Linux musl)
- `x86_64-pc-windows-msvc` (Windows)

每个构建作业：

1. 检出代码
2. 安装依赖（Node.js, pnpm, Rust）
3. 构建前端
4. 构建后端二进制文件
5. 使用 GPG 签名二进制文件
6. 打包为 zip（包含二进制文件、签名、README、LICENSE）
7. 上传为 artifact

### 2. Generate Changelog 作业

生成变更日志：

1. 获取上一个标签
2. 收集两个标签之间的提交
3. 提取 PR 编号
4. 从 Forgejo 获取 PR 标题
5. 如果是 GitHub PR，替换为 GitHub 标题
6. 格式化 changelog

### 3. Release 作业

创建 GitHub Release：

1. 下载所有构建 artifact
2. 创建 Release（使用生成的 changelog）
3. 上传 zip 文件

### 4. Upload S3 作业

上传到 S3：

1. 下载所有 artifact
2. 配置 AWS 凭证
3. 上传到 `s3://<bucket>/releases/<tag>/`

### 5. Build Container 作业

构建并推送容器镜像：

1. 登录 GHCR 和 Quay.io
2. 构建容器镜像
3. 推送标签：`:<tag>` 和 `:latest`
4. 推送到：
   - `ghcr.io/feniota/aphanite`
   - `quay.io/feniota/aphanite`

### 6. Sync Forgejo Release 作业

同步 Release 到 Forgejo：

1. 获取 GitHub Release 信息
2. 在 Forgejo 创建对应的 Release
3. 下载并上传所有 asset

---

## 测试工作流程

### 本地测试

1. 创建测试标签：

```bash
git tag -a v0.1.2-test -m "Test release"
git push origin v0.1.2-test
```

2. 在 GitHub Actions 页面查看工作流运行状态

3. 测试完成后删除标签：

```bash
git push origin :refs/tags/v0.1.2-test
gh release delete v0.1.2-test --yes
git tag -d v0.1.2-test
```

### 验证签名

下载 Release 后验证签名：

```bash
# 导入公钥
gpg --import aphanite-signing-key.asc

# 验证签名
gpg --verify aphanite-x86_64-unknown-linux-gnu.zip.sig aphanite-x86_64-unknown-linux-gnu.zip
```

---

## 故障排除

### GPG 导入失败

检查私钥格式是否正确：

```bash
# 验证密钥
gpg --list-packets /tmp/aphanite-signing-private.asc
```

### Forgejo API 错误

检查令牌权限和 API URL 是否正确：

```bash
# 测试 API
curl -H "Authorization: token $FORGEJO_TOKEN" \
  "$FORGEJO_BASE_URL/api/v1/user"
```

### R2 上传失败

检查 R2 凭证和存储桶权限：

```bash
# 测试凭证（需要安装 AWS CLI）
aws s3 ls s3://$R2_BUCKET/ --endpoint-url https://$R2_ACCOUNT_ID.r2.cloudflarestorage.com

# 检查存储桶访问
curl -I https://artifacts.feniota.org/
```

### 容器构建失败

检查 Dockerfile 和构建上下文：

```bash
# 本地构建测试
docker build -t aphanite:test .
```

---

## 安全注意事项

1. **永远不要**将私钥提交到代码库
2. 使用 GitHub Secrets 存储敏感信息
3. 定期轮换 API 令牌和密码
4. 限制 Forgejo 令牌权限（只需要 `repo` 权限）
5. 使用最小权限原则配置 R2 API 令牌
6. 考虑使用 Infisical 等密钥管理工具

---

## 后续优化

1. **添加 checksum 文件**: 为每个 release 生成 SHA256SUMS
2. **自动更新文档**: 更新下载链接到文档网站
3. **通知**: 添加 Slack/Discord 通知
4. **回滚机制**: 添加自动回滚失败 release 的功能
5. **缓存优化**: 优化 Rust 和 pnpm 缓存策略
