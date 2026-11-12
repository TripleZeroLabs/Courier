# Releasing Courier

Release builds are code-signed with
[Azure Artifact Signing](https://learn.microsoft.com/azure/artifact-signing/)
(formerly Trusted Signing), so Windows shows a verified publisher instead of
"Unknown publisher". Signing is optional for everyone else: without the
setup below, `npm run build` produces an unsigned installer.

## How signing is wired

- `src-tauri/tauri.conf.json` → `bundle.windows.signCommand` runs
  [`tools/sign.ps1`](../tools/sign.ps1) for every binary Tauri bundles:
  `courier.exe`, the uninstaller and the setup exe.
- `sign.ps1` uses SignTool with the Artifact Signing dlib and a timestamp
  from `http://timestamp.acs.microsoft.com` (Artifact Signing certificates
  last three days; the timestamp keeps signatures valid after that).
- Account details live in a `metadata.json` **outside the repo**: the path
  in `COURIER_SIGNING_METADATA`, else `ARTIFACT_SIGNING_METADATA` (a
  machine-wide setting other projects can share), else
  `%APPDATA%\ArtifactSigning\metadata.json`. The dlib path can be set the
  same way (`COURIER_SIGNING_DLIB` / `ARTIFACT_SIGNING_DLIB`).
- Authentication is your Azure sign-in (`az login`); no keys or secrets are
  stored anywhere.
- Without `metadata.json`, files are left unsigned and the build continues.
  With `COURIER_REQUIRE_SIGNING=1`, a missing or failed signature stops the
  build (use this for releases).

## One-time setup

### In Azure (portal.azure.com)

1. **Subscription:** an Azure subscription you can bill to.
2. **Resource provider:** Subscriptions → your subscription → Resource
   providers → register `Microsoft.CodeSigning`.
3. **Artifact Signing account:** create one (Basic SKU). Note its **name** and
   **region**.
4. **Identity validation:** give yourself the *Artifact Signing Identity
   Verifier* role on the account, then Identity validations → New →
   **Public**. An organization needs a verifiable legal history (Microsoft
   currently requires 3+ years); individuals are supported in some countries.
   Microsoft reviews the request; this can take days and may ask for
   documents.
5. **Certificate profile:** once validation is *Completed*, Certificate
   profiles → Create → **Public Trust**, using that validation. Note the
   profile **name**. The validated name becomes the publisher Windows shows.
6. **Signing permission:** give your own account the *Artifact Signing
   Certificate Profile Signer* role on the account (or the profile).

### On the signing PC

```powershell
winget install -e --id Microsoft.Azure.ArtifactSigningClientTools   # dlib, .NET 8, VC++ runtime
winget install -e --id Microsoft.AzureCLI                           # for az login
az login
```

SignTool comes with the Windows SDK (installed with the Visual Studio C++
Build Tools); version 10.0.22621.755 or later is required.

Set three user environment variables (they work for any project using the
same names):

```powershell
[Environment]::SetEnvironmentVariable('ARTIFACT_SIGNING_ENDPOINT', 'https://<region>.codesigning.azure.net/', 'User')
[Environment]::SetEnvironmentVariable('ARTIFACT_SIGNING_ACCOUNT',  '<account name>', 'User')
[Environment]::SetEnvironmentVariable('ARTIFACT_SIGNING_PROFILE',  '<certificate profile name>', 'User')
```

To sign as an app registration instead of your `az login`, also set
`AZURE_TENANT_ID`, `AZURE_CLIENT_ID` and `AZURE_CLIENT_SECRET`; that app
needs the *Artifact Signing Certificate Profile Signer* role.

Alternatively, create `%APPDATA%\ArtifactSigning\metadata.json` (or point
`ARTIFACT_SIGNING_METADATA` at one):

```json
{
  "Endpoint": "https://<region>.codesigning.azure.net",
  "CodeSigningAccountName": "<account name>",
  "CertificateProfileName": "<certificate profile name>"
}
```

The endpoint must match the account's region (for example East US →
`https://eus.codesigning.azure.net`, West Europe →
`https://weu.codesigning.azure.net`; full list in
[Microsoft's docs](https://learn.microsoft.com/azure/artifact-signing/how-to-signing-integrations#create-a-json-file)).
A mismatch causes `403 Forbidden` / `SignerSign() failed`.

Test it on any exe:

```powershell
powershell -ExecutionPolicy Bypass -File tools\sign.ps1 path\to\some.exe
```

## Cutting a release

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json`.
2. `cd src-tauri; cargo test`
3. Build signed:

   ```powershell
   $env:COURIER_REQUIRE_SIGNING = '1'; npm.cmd run build
   ```

4. Check the installer's signature:
   `Get-AuthenticodeSignature installer\Courier_<version>_x64-setup.exe`
   should say `Valid`.
5. Commit, tag and publish:

   ```powershell
   git commit -am "Release v<version>"
   git tag -a v<version> -m "Courier v<version>"
   git push origin main v<version>
   gh release create v<version> installer\Courier_<version>_x64-setup.exe --title "Courier v<version>" --notes-file <notes.md>
   ```

   Include the installer's SHA-256 (`Get-FileHash`) in the release notes.
