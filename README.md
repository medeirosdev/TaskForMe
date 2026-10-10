# TaskForMe

Notas, tarefas e lembretes organizados por projeto. É leve, roda offline e funciona no Linux e no Windows.

- **Projetos** com **divisões** (ex.: "Reuniões", "Backlog")
- **Tarefas** com **subtarefas**, data, lembrete e descrição
- **Notas** com salvamento automático, data e lembrete
- Filtros **Hoje** (inclui atrasadas), **Amanhã** e **Esta semana**, com tudo de todos os projetos
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
| Entrada inválida (data, cor, projeto) | Validada no backend; a interface não consegue gravar lixo |

### Onde ficam os dados

- Linux: `~/.local/share/com.medeiros.taskforme/`
- Windows: `%APPDATA%\com.medeiros.taskforme\`

Lá ficam `taskforme.db`, a pasta `backups/` e o `taskforme.log`. Em Configurações → "Abrir pasta" você chega direto.

## Testes

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # banco, recuperação, backups
npm run check                                      # tipos da interface
```

## Atalhos

| Atalho | Ação |
|---|---|
| `Ctrl N` | Nova tarefa (ou nova nota, na aba Notas) |
| `Ctrl 1` a `Ctrl 4` | Entrada, Hoje, Amanhã, Esta semana |
| `Esc` | Fecha o painel da tarefa |
| `Ctrl S` | Salva a nota na hora (ela já salva sozinha) |
