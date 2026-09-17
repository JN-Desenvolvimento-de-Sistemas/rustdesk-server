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
