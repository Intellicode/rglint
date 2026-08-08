export interface LintInput {
  schema?: string;
  documents: string[];
  rules?: Record<string, ["off" | "warn" | "error", Record<string, unknown>]>;
}

export interface LintResult {
  ruleId: string;
  message: string;
  line: number;
  column: number;
  filePath: string;
}

export interface ConfigProject {
  schema?: string | string[];
  documents?: string | string[];
  ignore: string[];
}

export interface Config {
  projects: Record<string, ConfigProject>;
  rules: Record<string, ["off" | "warn" | "error", Record<string, unknown>]>;
  ignore: string[];
  format: "pretty" | "json" | "sarif" | "github";
}

export function lint(input: LintInput): LintResult[];
export function loadConfig(path: string): Config;
