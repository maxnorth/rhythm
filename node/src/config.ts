/**
 * Configuration management for Rhythm
 */

export interface RhythmSettings {
  databaseUrl: string;
  defaultRetries: number;
  defaultTimeout: number;
  defaultWorkflowTimeout: number;
}

class Settings implements RhythmSettings {
  databaseUrl: string;
  defaultRetries: number;
  defaultTimeout: number;
  defaultWorkflowTimeout: number;

  constructor() {
    this.databaseUrl = process.env.DATABASE_URL || '';
    this.defaultRetries = parseInt(process.env.RHYTHM_DEFAULT_RETRIES || '3', 10);
    this.defaultTimeout = parseInt(process.env.RHYTHM_DEFAULT_TIMEOUT || '300', 10);
    this.defaultWorkflowTimeout = parseInt(
      process.env.RHYTHM_DEFAULT_WORKFLOW_TIMEOUT || '3600',
      10
    );
  }
}

export const settings = new Settings();
