# RustDesk Server — JN Desenvolvimento de Sistemas

Código-fonte público da nossa versão modificada do servidor RustDesk OSS (`hbbs` e `hbbr`). Este projeto é derivado do RustDesk, não é uma distribuição oficial nem implica afiliação com seus autores.

Licença: [GNU AGPL v3](LICENSE). Os avisos e o histórico do projeto original foram preservados. As alterações da JN estão identificadas em [CHANGES.md](CHANGES.md).

## Origem

- Servidor: https://github.com/rustdesk/rustdesk-server, commit `9bae9f2f39d92c4b4ba2e28e089da5071897b22e` (tag upstream `1.1.15`; o Cargo.toml original informa `1.1.14`).
- Biblioteca incorporada: https://github.com/rustdesk/hbb_common, commit `83419b6549636ee39dacef7776c473f5802e08d6`. Seu código está em `libs/hbb_common`, inclusive nos downloads ZIP; não é preciso inicializar submódulos.
- Documentação original: [README.upstream.md](README.upstream.md).

## Compilar e testar

Requer Docker com Buildx. A imagem produzida é Linux amd64. O Dockerfile utiliza Rust 1.90 e instala as dependências nativas; a compilação requer acesso aos repositórios públicos Debian, Docker e Cargo/Git identificados em `Cargo.lock`.

```sh
git clone https://github.com/JN-Desenvolvimento-de-Sistemas/rustdesk-server.git
cd rustdesk-server
docker buildx build --platform linux/amd64 --provenance=false --load -t rustdesk-server-jn:local .
```

O build executa `cargo test --locked --release --lib adm::tests` antes de compilar `hbbs` e `hbbr`. Em Debian Bookworm com Rust 1.90, protobuf-compiler, pkg-config, libssl-dev e libsodium-dev instalados:

```sh
SODIUM_USE_PKG_CONFIG=1 cargo test --locked --release --lib adm::tests
SODIUM_USE_PKG_CONFIG=1 cargo build --locked --release --bin hbbs --bin hbbr
```

O arquivo `.env` e o banco SQLite versionados são os arquivos de compilação do upstream, sem dados da nossa operação. `packaging/production.Dockerfile` e `packaging/server.patch` registram a receita original de build: para usá-la, execute o build com contexto `packaging` e `-f packaging/production.Dockerfile`. O Dockerfile da raiz compila diretamente o código publicado aqui.

## Configuração

Esta variante exige um serviço HTTPS de autorização que implemente [API-CONTRACT.md](API-CONTRACT.md). A implementação desse serviço não integra este repositório. Sem configuração válida ou em caso de falha na autorização, a conexão é negada. A compilação e os testes não precisam acessar a API de produção.

Configure ambos os processos com:

- `RUSTDESK_ADM_API`: URL HTTPS base do serviço compatível.
- `RUSTDESK_ADM_SECRET`: segredo compartilhado de pelo menos 32 caracteres, gerado e configurado pelo operador.
- `ALWAYS_USE_RELAY=Y`: configuração utilizada na nossa operação para encaminhar sessões por `hbbr`.

Exemplo em servidor Linux (substitua os domínios e crie `server.env` com as variáveis acima):

```sh
mkdir -p data
docker run -d --name hbbr --network host --env-file server.env -v "$PWD/data:/root" rustdesk-server-jn:local hbbr -k _
docker run -d --name hbbs --network host --env-file server.env -v "$PWD/data:/root" rustdesk-server-jn:local hbbs -r relay.example.org:21117 -k _
```

O diretório `data` guarda as chaves e o banco gerados na instalação. Configure no cliente oficial seu servidor ID, relay, chave pública e URL da API. Mantenha credenciais e chaves privadas fora do Git. Consulte a documentação original para portas e instalação. O controle desta variante aplica-se às conexões que passam por estes servidores.

## Acesso ao código

O código pode ser consultado, clonado e baixado gratuitamente, sem cadastro, neste repositório. As tags identificam as versões publicadas. Operadores desta versão modificada devem oferecer aos seus usuários de rede um link visível para o código correspondente à versão usada, conforme a seção 13 da licença.
