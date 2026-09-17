# Alterações desta distribuição

Modificações mantidas por JN Desenvolvimento de Sistemas. Publicação inicial: 2026-09-17. Código derivado do upstream identificado no README, sob GNU AGPL v3.

## jn-2026.09.17.1 — integração de autorização

- Consulta HTTPS à API para autorização de conexões; nega em caso de falha.
- Handshake TCP criptografado compatível com o cliente oficial e testes de protocolo.
- Registro de identidade/presença dos peers na API.
- Admissão dos participantes, revalidação e encerramento de sessões relay.
- Vínculo temporário entre autorização e resposta do destino; remoção de tokens antes de encaminhá-los ao destino.
- Encaminhamento por relay também em rede local quando `ALWAYS_USE_RELAY` está ativo.
- Uso do endereço efetivo da conexão, sem confiar em cabeçalhos WebSocket de IP encaminhado.
- Biblioteca `hbb_common` incorporada integralmente para permitir compilação do download ZIP.
- Receita de compilação, contrato HTTP e testes disponibilizados junto ao código.

A imagem com digest `sha256:1161bbbb70595c0839f93d27dd0097ca44d531128e26b47ba122bcb46479d531` usou esta versão de código. Em 2026-09-17 o processo hbbr continuou usando essa imagem.

## jn-2026.09.17.2 — mensagem de permissão

Alteração de 2026-09-17 em `src/adm.rs` e `src/rendezvous_server.rs`: negativas de autorização passam a usar `PunchHoleResponse.other_failure` com a mensagem “Você não possui permissão para acessar esse dispositivo. Entre em contato com o administrador”. O campo de endereço fica vazio e a conexão permanece bloqueada. O erro de chave continua reservado à checagem da chave. O teste de protocolo verifica a mensagem UTF-8 na resposta criptografada.

A imagem com digest `sha256:78c8560d41465c0bbdb3631134a9bced2a32ecfc70860ad15e17133b67ed1645` usou esta versão de código; aplicada ao hbbs em 2026-09-17. O cliente oficial não foi modificado.


## jn-2026.09.17.3 — atualização OSS e diagnóstico de autorização

Alterações de 2026-09-17:

- Base atualizada para RustDesk OSS 1.1.16 (`73523b31cfd25d77dee862e6fc9f5e1fb5e485ef`), preservando a integração JN e a biblioteca incorporada.
- Correção adicional upstream `109d9a235136c883f544cb3ea0f11a58f1cedd58`: desconsiderar PunchHoleSent e LocalAddr por UDP, além de PunchHoleRequest, para evitar reflexão/amplificação. Registro UDP de dispositivos permanece disponível.
- Correção upstream de overflow no tempo de presença: dispositivos offline por aproximadamente 25–50 dias não reaparecem online.
- Respostas `allow: false` preservam a mensagem de permissão. Falha de configuração, timeout, HTTP não 2xx ou resposta inválida recebem “Serviço temporariamente indisponível. Tente novamente em instantes”. Ambos os casos permanecem bloqueados.
- Mensagens enviadas pelos campos já suportados pelo cliente oficial em PunchHoleResponse e RelayResponse. Nenhuma alteração no cliente.
- Categorias técnicas registradas sem tokens, segredo, URL ou corpo da resposta.
- Testes de respostas HTTP, timeout, transporte, mensagens criptografadas, UDP e presença antiga. O build agora executa todos os testes da biblioteca.

Validação desta versão: nove testes da biblioteca aprovados; registro UDP e respostas criptografadas de indisponibilidade para PunchHoleRequest e RequestRelay confirmados com o binário compilado em ambiente isolado. Imagem publicada: `sha256:99435df34a0290c75137b2d43561c5e42264ce2045958e99667ed8f9eec7a9ea`.
