import type { AppMessages } from "../index";

/** Firebird PowerToys — mensagens específicas do app (Português Brasileiro). */
const ptBR: AppMessages = {
  // App
  appTitle: "Firebird PowerToys",
  connected: "Conectado",
  connecting: "Conectando…",
  disconnected: "Desconectado",
  connectionError: "Erro de conexão",
  reconnect: "Reconectar",

  // Query page
  query: "Consulta",
  run: "Executar",
  runHint: "Executa a instrução SQL (Ctrl+Enter)",
  explainPlan: "Plano de Execução",
  explainPlanHint: "Mostra o plano de execução (a instrução é revertida)",
  database: "Banco de dados",
  selectDatabase: "Selecione um banco de dados…",
  sqlEditorPlaceholder: "SELECT * FROM RDB$RELATIONS WHERE RDB$SYSTEM_FLAG = 0",
  results: "Resultados",
  rows: "linhas",
  truncated: "Resultados truncados — use a paginação para buscar a próxima página.",
  noResults: "Nenhum resultado.",
  executionPlan: "Plano de execução",
  dmlResult: "Instrução executada",
  page: "Página",
  of: "de",
  nextPage: "Próxima página",
  prevPage: "Página anterior",

  // Schema page
  schema: "Estrutura",
  tables: "Tabelas",
  views: "Views",
  procedures: "Procedimentos",
  functions: "Funções",
  domains: "Domains",
  triggers: "Triggers",
  indexes: "Índices",
  sequences: "Sequências",
  object: "Objeto",
  columns: "Colunas",
  constraints: "Constraints",
  ddl: "DDL",
  extractDdl: "Extrair DDL",
  extractDdlHint: "Extrai o DDL (instruções CREATE) do objeto selecionado",
  noObjectSelected: "Selecione um objeto para ver os detalhes.",
  name: "Nome",
  type: "Tipo",
  nullable: "Nulo",
  defaultValue: "Padrão",
  position: "Posição",
  domain: "Domain",
  unique: "Único",
  timing: "Timing",
  event: "Evento",
  table: "Tabela",
  description: "Descrição",
  parameters: "Parâmetros",
  returns: "Retorna",

  // Errors
  loadError: "Falha ao carregar dados",
  serverError: "Erro no servidor",
};

export default ptBR;
