Prévia do Monitor de RAM para Windows x64.

- O MSI foi compilado no GitHub Actions a partir da tag desta versão. Confira a origem pelo comando abaixo e a integridade pelo arquivo SHA-256.
- O instalador ainda está sem assinatura Windows Authenticode.
- O monitoramento vem ativado por padrão. A limpeza é uma opção restrita a processos ociosos autorizados.

```powershell
gh attestation verify .\MONITOR_MSI --repo themarcosmail-bot/monitor-de-ram-windows --signer-workflow themarcosmail-bot/monitor-de-ram-windows/.github/workflows/release.yml --deny-self-hosted-runners
```

[Code signing policy](https://github.com/themarcosmail-bot/monitor-de-ram-windows/blob/main/README.md#code-signing-policy)
