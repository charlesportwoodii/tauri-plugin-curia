import { invoke } from "@tauri-apps/api/core";

export type LogValue =
  | string
  | number
  | boolean
  | null
  | LogValue[]
  | { [key: string]: LogValue };

export type LogFields = Record<string, LogValue>;

export enum LogLevel {
  Trace = "trace",
  Debug = "debug",
  Info = "info",
  Warn = "warn",
  Error = "error",
}

/**
 * The caller's file and line, recovered from a synthetic stack frame. Best
 * effort: a minified bundle yields nothing useful and the fields stay undefined.
 */
function callerLocation(): {
  location?: string;
  file?: string;
  line?: number;
} {
  const stack = new Error().stack?.split("\n");
  if (!stack || stack.length < 4) {
    return {};
  }

  const match = stack[3].match(/at\s+(?:(.+?)\s+\()?(.+?):(\d+):\d+\)?$/);
  if (!match) {
    return {};
  }

  return {
    location: match[1],
    file: match[2],
    line: Number.parseInt(match[3], 10),
  };
}

async function emit(
  level: LogLevel,
  message: string,
  fields?: LogFields,
): Promise<void> {
  const { location, file, line } = callerLocation();

  await invoke("plugin:curia|log", {
    record: { level, message, fields, location, file, line },
  });
}

export async function error(
  message: string,
  fields?: LogFields,
): Promise<void> {
  await emit(LogLevel.Error, message, fields);
}

export async function warn(message: string, fields?: LogFields): Promise<void> {
  await emit(LogLevel.Warn, message, fields);
}

export async function info(message: string, fields?: LogFields): Promise<void> {
  await emit(LogLevel.Info, message, fields);
}

export async function debug(
  message: string,
  fields?: LogFields,
): Promise<void> {
  await emit(LogLevel.Debug, message, fields);
}

export async function trace(
  message: string,
  fields?: LogFields,
): Promise<void> {
  await emit(LogLevel.Trace, message, fields);
}
