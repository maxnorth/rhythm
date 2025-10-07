/**
 * Configuration management for Currant
 */

export interface CurrantSettings {
  databaseUrl: string;
  defaultRetries: number;
  defaultTimeout: number;
  defaultWorkflowTimeout: number;
}

class Settings implements CurrantSettings {
  databaseUrl: string;
  defaultRetries: number;
  defaultTimeout: number;
  defaultWorkflowTimeout: number;

  constructor() {
    this.databaseUrl = process.env.DATABASE_URL || '';
    this.defaultRetries = parseInt(process.env.CURRANT_DEFAULT_RETRIES || '3', 10);
    this.defaultTimeout = parseInt(process.env.CURRANT_DEFAULT_TIMEOUT || '300', 10);
    this.defaultWorkflowTimeout = parseInt(
      process.env.CURRANT_DEFAULT_WORKFLOW_TIMEOUT || '3600',
      10
    );
  }
}

export const settings = new Settings();
