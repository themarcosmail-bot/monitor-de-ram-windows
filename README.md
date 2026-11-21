# Monitor de RAM

Monitor de memória para Windows em Rust, criado por [@themarcosmail-bot](https://github.com/themarcosmail-bot). A interface mostra a RAM disponível, o horário da última leitura e o resultado da última ação.

## Instalação e uso

Baixe o MSI para Windows x64 na página de [Releases](https://github.com/themarcosmail-bot/monitor-de-ram-windows/releases). O instalador cria um atalho no menu Iniciar e instala para o usuário atual. Abra o **Monitor de RAM** e configure o intervalo entre **1 e 1440 minutos**. O padrão é **1 minuto**. O timer funciona enquanto o app está aberto, inclusive minimizado.

A instalação inicia no modo **monitoramento**. Para testar a limpeza automática:

1. Abra um programa cujo conjunto residente você queira reduzir quando estiver ocioso.
2. No monitor, clique em **Atualizar lista**, selecione o programa e clique em **Autorizar**.
3. Marque **Limpar automaticamente abaixo do limite**, escolha o limite e clique em **Aplicar**. O limite inicial é **1024 MB**.

A limpeza exige duas leituras consecutivas abaixo do limite. A cada ação, o app tenta limpar apenas um processo do mesmo usuário e sessão, que tenha pelo menos 96 MB residentes, esteja ocioso e cujo executável autorizado não tenha uma janela visível em uso. Há 10 minutos de intervalo entre tentativas na mesma execução. **Verificar agora** só faz uma leitura.

A operação usa `EmptyWorkingSet` do Windows. Ela retira páginas residentes do processo escolhido. Ao voltar ao programa, o Windows pode recarregar essas páginas, com possível atraso e atividade de disco. A redução exibida pertence ao processo e pode ser temporária. Para decidir se a opção ajuda, compare o mesmo uso com a limpeza desligada e ligada e avalie a resposta dos programas, além da RAM disponível. [Como o Windows gerencia conjuntos residentes](https://learn.microsoft.com/windows/win32/memory/working-set)

## Privacidade e distribuição

O app funciona localmente, sem acesso à rede. O arquivo `%LOCALAPPDATA%\Monitor de RAM\config.txt` registra o intervalo, o limite e os caminhos dos executáveis autorizados. Esse arquivo pode ser removido separadamente após a desinstalação. O app é executado com as permissões do usuário e usa interfaces documentadas do Windows.

Para desinstalar o programa, use **Configurações → Aplicativos → Aplicativos instalados → Monitor de RAM → Desinstalar**.

## Code signing policy

A partir da versão 1.0.1, o GitHub Actions compila o MSI a partir da tag da release e registra uma atestação de procedência. O SHA-256 verifica os bytes do arquivo. A atestação registra qual repositório e fluxo produziram o MSI. As prévias estão sem assinatura Windows Authenticode. [Atestações do GitHub](https://docs.github.com/en/actions/concepts/security/artifact-attestations)

```powershell
Get-FileHash .\Monitor-de-RAM-1.0.1-x64.msi -Algorithm SHA256
gh attestation verify .\Monitor-de-RAM-1.0.1-x64.msi --repo themarcosmail-bot/monitor-de-ram-windows --signer-workflow themarcosmail-bot/monitor-de-ram-windows/.github/workflows/release.yml --deny-self-hosted-runners
```

O mantenedor do repositório revisa contribuições e publica as tags de lançamento. Para o Authenticode gratuito, o projeto pode solicitar avaliação à SignPath Foundation, que define a elegibilidade e exige aprovação manual para a assinatura de cada release. [Condições da SignPath Foundation](https://signpath.org/terms)

## Teste de desempenho

O GitHub Actions executa testes em VMs de Windows Server 2022 e 2025. Um teste A/B compara cinco execuções por modo em processos de laboratório de 128 MiB. Ele mede a redução da memória residente e o tempo para reler as páginas ao retomar. Veja o resumo em [Actions](https://github.com/themarcosmail-bot/monitor-de-ram-windows/actions/workflows/ci.yml). A carga sintética mede o custo de retomar páginas e não representa a fluidez de aplicativos em uso diário.

## Compilar e empacotar

Em Windows x64, use Rust estável com a ferramenta de compilação do seu alvo e o WiX Toolset 3.14.1 portátil:

```powershell
cargo test --locked
.\build-msi.ps1 -WixBin 'C:\caminho\para\wix'
```

O MSI e seu SHA-256 são gravados em `dist\`. O alvo padrão do script é `x86_64-pc-windows-msvc`. Para GNU x64, passe `-Target x86_64-pc-windows-gnu` e disponibilize o linker e o dlltool do seu ambiente.

Licença [MIT](LICENSE).
