# Contrato do serviço de autorização

Contrato implementado pelo código de integração deste repositório. Todos os endpoints recebem POST com JSON sob `{RUSTDESK_ADM_API}/api/internal/rustdesk/v1/` e o cabeçalho `X-RustDesk-Server-Secret`. A URL deve usar HTTPS; redirecionamentos não são seguidos. Timeout de conexão: 2 segundos; total: 4 segundos.

Somente uma resposta HTTP 2xx com JSON contendo `"allow": true` é aceita como autorização. Para autorização, uma resposta 2xx com `"allow": false` representa falta de permissão. Erros HTTP (incluindo 403 do segredo de servidor, 429 e 5xx), timeout, falha de transporte/TLS, JSON inválido ou campo booleano ausente representam indisponibilidade. Ambos os casos bloqueiam o acesso; apenas a mensagem e a categoria do diagnóstico diferem. A API deve validar o segredo e decidir permissões a partir de dados confiáveis; não basta devolver `allow` indiscriminadamente.

| Endpoint | Corpo enviado | Finalidade |
| --- | --- | --- |
| `authorize-connection` | `token`, `destination_id`, `destination_identity`, `attempt_id`; na autorização de relay também `relay_uuid`, `source_ip`, `destination_ip` | Autorizar a tentativa e, quando aplicável, vincular a sessão relay |
| `peers` | `peers`: lista com `id`, `uuid`, `fingerprint`, `seen_seconds_ago` | Informar dispositivos registrados recentemente; lotes de até 200, a cada 20 segundos |
| `relay/{uuid}/claim` | `ip` | Admitir cada participante no relay; até 10 tentativas com intervalo de 200 ms |
| `relay/{uuid}/status` | `{}` | Revalidar a autorização da sessão; consulta a cada 5 segundos após a resposta anterior |
| `relay/{uuid}/close` | `{}` | Informar o encerramento da sessão |

`destination_identity` contém `uuid` em base64 e `fingerprint` SHA-256 hexadecimal da chave pública registrada. Identidades sem chave/UUID ou com registro há mais de 60 segundos não são consideradas atuais. `attempt_id` é um UUID gerado para a requisição. `relay_uuid` identifica o pareamento; a API precisa correlacioná-lo com a autorização e com os participantes, e tratar concorrência/repetição.

O encerramento (`relay/{uuid}/close`) pode responder `{"closed": true}`; essa notificação não concede autorização. O servidor registra somente a operação e a categoria da falha, sem corpos de requisição, tokens, segredo ou URL. A integração não mantém cache das decisões.

A API é responsável por autenticação de usuários, validade do token, permissões atuais, cadastro/aprovação de dispositivos e estado das sessões. Este documento descreve a interface pública utilizada pelo servidor; não contém usuários, tokens ou regras privadas de uma implantação.
