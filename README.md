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

O MSI inicial traz o crédito de autoria e um SHA-256 publicado na release. Ele ainda não possui assinatura Authenticode. O SHA-256 verifica a integridade do arquivo. Para distribuição mais ampla, uma próxima etapa é assinar o EXE e o MSI com um certificado de assinatura de código confiável e carimbo de tempo. [Opções de assinatura da Microsoft](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options)

```powershell
Get-FileHash .\Monitor-de-RAM-1.0.0-x64.msi -Algorithm SHA256
```

## Compilar e empacotar

Em Windows x64, use Rust estável com a ferramenta de compilação do seu alvo e o WiX Toolset 3.14.1 portátil:

```powershell
cargo test --locked
.\build-msi.ps1 -WixBin 'C:\caminho\para\wix'
```

O MSI e seu SHA-256 são gravados em `dist\`. O alvo padrão do script é `x86_64-pc-windows-msvc`. Para GNU x64, passe `-Target x86_64-pc-windows-gnu` e disponibilize o linker e o dlltool do seu ambiente.

Licença [MIT](LICENSE).
