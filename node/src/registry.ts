/**
 * Function registry for dynamic execution
 */

type ExecutableFunction = (...args: any[]) => Promise<any>;

class FunctionRegistry {
  private functions: Map<string, ExecutableFunction> = new Map();

  register(name: string, fn: ExecutableFunction): void {
    this.functions.set(name, fn);
  }

  get(name: string): ExecutableFunction | undefined {
    return this.functions.get(name);
  }

  has(name: string): boolean {
    return this.functions.has(name);
  }

  list(): string[] {
    return Array.from(this.functions.keys());
  }
}

export const registry = new FunctionRegistry();

export function registerFunction(name: string, fn: ExecutableFunction): void {
  registry.register(name, fn);
}
