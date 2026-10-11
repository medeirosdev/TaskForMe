# TaskForMe

Notas, tarefas e lembretes organizados por projeto. É leve, roda offline e funciona no Linux e no Windows.

- **Projetos** com **divisões** (ex.: "Reuniões", "Backlog")
- **Tarefas** com **subtarefas**, data, lembrete e descrição
- **Notas** com salvamento automático, data e lembrete
- **Eventos** de um dia ou de um período, com horário ou dia inteiro e **aviso** (na hora, 15 min, 1 h, 1 dia ou 1 semana antes)
- Tela **Todos os eventos**, com os eventos de todos os projetos agrupados por mês
- **Links úteis** por projeto (nome, descrição e endereço). Um clique abre no navegador
- Filtros **Hoje** (inclui atrasadas), **Amanhã**, **Esta semana** e **Próxima semana** (de domingo a domingo), com tarefas, notas e eventos de todos os projetos
- Lembretes com notificação nativa. O app fica na bandeja para os lembretes continuarem funcionando
- Tema escuro e claro, baseado no design system do Linear (`design/DESIGN.md`)

## Rodar em desenvolvimento

Pré-requisitos: Node 20+, Rust estável e, no Linux, as libs do WebKitGTK:

```bash
# Ubuntu/Debian
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```

```bash
npm install
npm run app        # abre o app com hot-reload
```

O modo de desenvolvimento usa outro identificador (`com.medeiros.taskforme.dev`, configurado em
`src-tauri/tauri.dev.conf.json`). Ele tem os próprios dados e roda ao lado do app instalado sem misturar nada.

## Gerar instaladores

```bash
npm run bundle     # Linux: .deb, .rpm e .AppImage em src-tauri/target/release/bundle/
```

Para o **Windows**, rode o workflow **Build** no GitHub Actions (aba Actions → Build → Run workflow).
Ele gera o `.exe` e o `.msi` sem você precisar de uma máquina Windows. No Windows também dá para rodar
`npm run bundle` direto.

## Arquitetura

```
src/                    Interface (Svelte 5 + TypeScript)
  App.svelte            Layout, atalhos, tema, tratamento global de erros
  lib/api.ts            Única porta para o backend; todo erro vira aviso na tela
  lib/store.svelte.ts   Estado global, autosave e "Desfazer"
  components/           Telas e componentes
src-tauri/              Backend (Rust + Tauri 2)
  src/db.rs             SQLite: migrations, backup automático, recuperação de falhas
  src/commands.rs       Comandos chamados pela interface (com validação de tudo)
  src/reminders.rs      Thread que dispara os lembretes
  src/lib.rs            Bandeja, instância única, inicialização
```

### Proteção contra falhas

| Situação | Comportamento |
|---|---|
| PC desliga no meio de uma escrita | SQLite em WAL + `synchronous=FULL`: nada confirmado se perde e o banco não corrompe |
| Banco corrompido | Na abertura roda `quick_check`. O arquivo ruim é **preservado** (`taskforme.corrupt-*.db`) e o backup mais recente é restaurado. Você é avisado |
| Backups | Automático uma vez por dia (também com o app aberto na bandeja), antes de cada migration e pelo botão em Configurações. Ficam os últimos 14 |
| Excluiu sem querer | Toda exclusão tem **Desfazer**. Os itens só são apagados de vez depois de 7 dias |
| Fechou com nota não salva | Autosave a cada pausa na digitação. Ao sair, o app salva o pendente antes de fechar |
| Falha ao salvar | O texto continua na tela e o app tenta salvar de novo sozinho |
| Erro inesperado na interface | Aparece um aviso, sem tela branca. Os dados ficam no backend |
| Dois cliques no ícone | Instância única: abre a janela que já está aberta, sem dois processos no mesmo banco |
| Banco de uma versão mais nova | O app se recusa a abrir em vez de "consertar" e perder dados |
| Entrada inválida (data, cor, projeto, link) | Validada no backend; a interface não consegue gravar lixo |
| Link malicioso (`javascript:`, `file:`) | Só endereços `http(s)` são aceitos e abertos |

### Backup no GitHub (opcional)

Em **Configurações → Backup no GitHub**, informe o endereço de um repositório **privado**
(ex.: `git@github.com:usuario/taskforme-dados.git`) e ative a opção. O app então:

- exporta os dados como `taskforme.sql` (texto: o histórico do Git mostra o que mudou e ocupa pouco espaço);
- faz commit e push **ao abrir** (com "Iniciar com o sistema" ligado, isso acontece ao ligar o PC) e **uma vez por dia** se ficar aberto;
- usa a sua identidade e as suas credenciais do Git (chave SSH ou login salvo). Nunca pede senha;
- se falhar (sem internet, sem permissão), tenta de novo em 2, 10, 30 e 60 minutos e mostra o motivo em Configurações.

O próprio repositório recebe um `README.md` com as instruções de restauração.

### Onde ficam os dados

- Linux: `~/.local/share/com.medeiros.taskforme/`
- Windows: `%APPDATA%\com.medeiros.taskforme\`

Lá ficam `taskforme.db`, a pasta `backups/` e o `taskforme.log`. Em Configurações → "Abrir pasta" você chega direto.

## Testes

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # banco, recuperação, backups, eventos, sync
npm run check                                      # tipos da interface
```

## Atalhos

| Atalho | Ação |
|---|---|
| `Ctrl N` | Novo item da aba aberta (tarefa, nota, evento ou link) |
| `Ctrl 1` a `Ctrl 6` | Entrada, Hoje, Amanhã, Esta semana, Próxima semana, Todos os eventos |
| `Esc` | Fecha o painel da tarefa |
| `Ctrl S` | Salva a nota na hora (ela já salva sozinha) |
