# Segurança

Para relatar uma vulnerabilidade, use [o canal privado do GitHub](https://github.com/themarcosmail-bot/monitor-de-ram-windows/security/advisories/new). Inclua a versão do app, a versão do Windows e passos de reprodução. Remova caminhos pessoais, nomes de documentos e outros dados privados dos anexos.

O aplicativo inicia em modo monitoramento. A limpeza automática depende de ativação explícita e de uma lista de caminhos autorizados. A propriedade do processo, a sessão, a identidade do executável, o horário de criação e a atividade recente são verificados antes da operação. O aplicativo não solicita privilégios administrativos.

Os resultados de memória variam entre máquinas e cargas. Um conjunto residente menor pode causar recarregamentos ao retomar um programa. Se a resposta piorar, volte ao modo monitoramento.
