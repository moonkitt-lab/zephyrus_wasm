export enum ErrorCode {
  // Configuration Errors
  CONFIG_INVALID = "CONFIG_INVALID",
  CONFIG_MISSING = "CONFIG_MISSING",
  
  // Wallet Errors
  WALLET_CREATE_FAILED = "WALLET_CREATE_FAILED",
  WALLET_FUNDING_FAILED = "WALLET_FUNDING_FAILED",
  WALLET_INSUFFICIENT_FUNDS = "WALLET_INSUFFICIENT_FUNDS",
  
  // Contract Errors
  CONTRACT_NOT_FOUND = "CONTRACT_NOT_FOUND",
  CONTRACT_CALL_FAILED = "CONTRACT_CALL_FAILED",
  CONTRACT_QUERY_FAILED = "CONTRACT_QUERY_FAILED",
  CONTRACT_INSTANTIATE_FAILED = "CONTRACT_INSTANTIATE_FAILED",
  
  // Transaction Errors
  TX_FAILED = "TX_FAILED",
  TX_TIMEOUT = "TX_TIMEOUT",
  TX_INSUFFICIENT_GAS = "TX_INSUFFICIENT_GAS",
  
  // Scenario Errors
  SCENARIO_INVALID = "SCENARIO_INVALID",
  SCENARIO_EXECUTION_FAILED = "SCENARIO_EXECUTION_FAILED",
  
  // Validation Errors
  REWARDS_MISMATCH = "REWARDS_MISMATCH",
  VALIDATION_FAILED = "VALIDATION_FAILED",
  
  // Network Errors
  NETWORK_UNREACHABLE = "NETWORK_UNREACHABLE",
  RPC_ERROR = "RPC_ERROR",
  
  // File System Errors
  FILE_NOT_FOUND = "FILE_NOT_FOUND",
  FILE_READ_ERROR = "FILE_READ_ERROR",
  FILE_WRITE_ERROR = "FILE_WRITE_ERROR",
  
  // Generic Errors
  UNKNOWN_ERROR = "UNKNOWN_ERROR",
  TIMEOUT_ERROR = "TIMEOUT_ERROR"
}

export class ZephyrusTestError extends Error {
  public readonly code: ErrorCode;
  public readonly details?: any;
  public readonly timestamp: Date;
  public readonly context?: string;

  constructor(
    code: ErrorCode,
    message: string,
    details?: any,
    context?: string
  ) {
    super(message);
    this.name = "ZephyrusTestError";
    this.code = code;
    this.details = details;
    this.timestamp = new Date();
    this.context = context;
  }

  toString(): string {
    let errorString = `[${this.code}] ${this.message}`;
    
    if (this.context) {
      errorString = `${errorString} (Context: ${this.context})`;
    }
    
    if (this.details) {
      errorString = `${errorString}\nDetails: ${JSON.stringify(this.details, null, 2)}`;
    }
    
    return errorString;
  }

  toJSON() {
    return {
      name: this.name,
      code: this.code,
      message: this.message,
      context: this.context,
      details: this.details,
      timestamp: this.timestamp.toISOString(),
      stack: this.stack
    };
  }
}

export class ErrorHandler {
  private static instance: ErrorHandler;
  private errorLog: ZephyrusTestError[] = [];
  
  static getInstance(): ErrorHandler {
    if (!ErrorHandler.instance) {
      ErrorHandler.instance = new ErrorHandler();
    }
    return ErrorHandler.instance;
  }

  handleError(error: any, context?: string): ZephyrusTestError {
    let testError: ZephyrusTestError;

    if (error instanceof ZephyrusTestError) {
      testError = error;
    } else {
      testError = this.convertToTestError(error, context);
    }

    this.errorLog.push(testError);
    return testError;
  }

  private convertToTestError(error: any, context?: string): ZephyrusTestError {
    const message = error.message || String(error);
    let code = ErrorCode.UNKNOWN_ERROR;

    // Categorize error based on message content
    if (message.includes("contract") && message.includes("not found")) {
      code = ErrorCode.CONTRACT_NOT_FOUND;
    } else if (message.includes("insufficient funds") || message.includes("insufficient balance")) {
      code = ErrorCode.WALLET_INSUFFICIENT_FUNDS;
    } else if (message.includes("gas")) {
      code = ErrorCode.TX_INSUFFICIENT_GAS;
    } else if (message.includes("timeout")) {
      code = ErrorCode.TX_TIMEOUT;
    } else if (message.includes("network") || message.includes("connection")) {
      code = ErrorCode.NETWORK_UNREACHABLE;
    } else if (message.includes("rpc") || message.includes("RPC")) {
      code = ErrorCode.RPC_ERROR;
    } else if (message.includes("file") && message.includes("not found")) {
      code = ErrorCode.FILE_NOT_FOUND;
    } else if (message.includes("scenario") && message.includes("invalid")) {
      code = ErrorCode.SCENARIO_INVALID;
    } else if (message.includes("transaction failed")) {
      code = ErrorCode.TX_FAILED;
    }

    return new ZephyrusTestError(code, message, error, context);
  }

  createError(code: ErrorCode, message: string, details?: any, context?: string): ZephyrusTestError {
    return new ZephyrusTestError(code, message, details, context);
  }

  getErrorLog(): ZephyrusTestError[] {
    return [...this.errorLog];
  }

  clearErrorLog(): void {
    this.errorLog = [];
  }

  getErrorsByCode(code: ErrorCode): ZephyrusTestError[] {
    return this.errorLog.filter(error => error.code === code);
  }

  hasErrors(): boolean {
    return this.errorLog.length > 0;
  }

  hasCriticalErrors(): boolean {
    const criticalCodes = [
      ErrorCode.CONTRACT_NOT_FOUND,
      ErrorCode.NETWORK_UNREACHABLE,
      ErrorCode.WALLET_CREATE_FAILED,
      ErrorCode.CONFIG_INVALID
    ];
    
    return this.errorLog.some(error => criticalCodes.includes(error.code));
  }

  generateErrorSummary(): string {
    if (this.errorLog.length === 0) {
      return "No errors recorded";
    }

    const errorsByCode = new Map<ErrorCode, number>();
    for (const error of this.errorLog) {
      const count = errorsByCode.get(error.code) || 0;
      errorsByCode.set(error.code, count + 1);
    }

    let summary = `Error Summary (${this.errorLog.length} total errors):\n`;
    for (const [code, count] of errorsByCode) {
      summary += `  ${code}: ${count}\n`;
    }

    return summary;
  }
}

// Utility functions for common error scenarios
export function handleAsyncError<T>(
  promise: Promise<T>,
  context?: string
): Promise<T> {
  return promise.catch(error => {
    const testError = ErrorHandler.getInstance().handleError(error, context);
    throw testError;
  });
}

export function wrapWithErrorHandling<T extends any[], R>(
  fn: (...args: T) => R,
  context?: string
): (...args: T) => R {
  return (...args: T): R => {
    try {
      return fn(...args);
    } catch (error) {
      const testError = ErrorHandler.getInstance().handleError(error, context);
      throw testError;
    }
  };
}

export function wrapAsyncWithErrorHandling<T extends any[], R>(
  fn: (...args: T) => Promise<R>,
  context?: string
): (...args: T) => Promise<R> {
  return async (...args: T): Promise<R> => {
    try {
      return await fn(...args);
    } catch (error) {
      const testError = ErrorHandler.getInstance().handleError(error, context);
      throw testError;
    }
  };
}

// Validation helper functions
export function validateRequired<T>(value: T | null | undefined, name: string, context?: string): T {
  if (value === null || value === undefined) {
    throw ErrorHandler.getInstance().createError(
      ErrorCode.CONFIG_MISSING,
      `Required value '${name}' is missing`,
      { name, value },
      context
    );
  }
  return value;
}

export function validatePositive(value: number, name: string, context?: string): number {
  if (value <= 0) {
    throw ErrorHandler.getInstance().createError(
      ErrorCode.CONFIG_INVALID,
      `Value '${name}' must be positive, got: ${value}`,
      { name, value },
      context
    );
  }
  return value;
}

export function validateArray<T>(value: any, name: string, context?: string): T[] {
  if (!Array.isArray(value)) {
    throw ErrorHandler.getInstance().createError(
      ErrorCode.CONFIG_INVALID,
      `Value '${name}' must be an array, got: ${typeof value}`,
      { name, value, type: typeof value },
      context
    );
  }
  return value as T[];
}

export function validateString(value: any, name: string, context?: string): string {
  if (typeof value !== "string") {
    throw ErrorHandler.getInstance().createError(
      ErrorCode.CONFIG_INVALID,
      `Value '${name}' must be a string, got: ${typeof value}`,
      { name, value, type: typeof value },
      context
    );
  }
  return value;
}

export function validateNumber(value: any, name: string, context?: string): number {
  if (typeof value !== "number" || isNaN(value)) {
    throw ErrorHandler.getInstance().createError(
      ErrorCode.CONFIG_INVALID,
      `Value '${name}' must be a valid number, got: ${value}`,
      { name, value, type: typeof value },
      context
    );
  }
  return value;
}

// Retry mechanism with error handling
export async function retryWithErrorHandling<T>(
  operation: () => Promise<T>,
  maxRetries: number = 3,
  delay: number = 1000,
  context?: string
): Promise<T> {
  let lastError: Error;

  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      return await operation();
    } catch (error) {
      lastError = error as Error;
      
      if (attempt === maxRetries) {
        const testError = ErrorHandler.getInstance().handleError(
          lastError,
          `${context} (after ${maxRetries} retries)`
        );
        throw testError;
      }

      // Wait before retrying
      await new Promise(resolve => setTimeout(resolve, delay * attempt));
    }
  }

  // This should never be reached, but TypeScript requires it
  throw lastError!;
}