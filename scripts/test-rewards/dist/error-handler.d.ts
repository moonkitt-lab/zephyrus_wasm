export declare enum ErrorCode {
    CONFIG_INVALID = "CONFIG_INVALID",
    CONFIG_MISSING = "CONFIG_MISSING",
    WALLET_CREATE_FAILED = "WALLET_CREATE_FAILED",
    WALLET_FUNDING_FAILED = "WALLET_FUNDING_FAILED",
    WALLET_INSUFFICIENT_FUNDS = "WALLET_INSUFFICIENT_FUNDS",
    CONTRACT_NOT_FOUND = "CONTRACT_NOT_FOUND",
    CONTRACT_CALL_FAILED = "CONTRACT_CALL_FAILED",
    CONTRACT_QUERY_FAILED = "CONTRACT_QUERY_FAILED",
    CONTRACT_INSTANTIATE_FAILED = "CONTRACT_INSTANTIATE_FAILED",
    TX_FAILED = "TX_FAILED",
    TX_TIMEOUT = "TX_TIMEOUT",
    TX_INSUFFICIENT_GAS = "TX_INSUFFICIENT_GAS",
    SCENARIO_INVALID = "SCENARIO_INVALID",
    SCENARIO_EXECUTION_FAILED = "SCENARIO_EXECUTION_FAILED",
    REWARDS_MISMATCH = "REWARDS_MISMATCH",
    VALIDATION_FAILED = "VALIDATION_FAILED",
    NETWORK_UNREACHABLE = "NETWORK_UNREACHABLE",
    RPC_ERROR = "RPC_ERROR",
    FILE_NOT_FOUND = "FILE_NOT_FOUND",
    FILE_READ_ERROR = "FILE_READ_ERROR",
    FILE_WRITE_ERROR = "FILE_WRITE_ERROR",
    UNKNOWN_ERROR = "UNKNOWN_ERROR",
    TIMEOUT_ERROR = "TIMEOUT_ERROR"
}
export declare class ZephyrusTestError extends Error {
    readonly code: ErrorCode;
    readonly details?: any;
    readonly timestamp: Date;
    readonly context?: string;
    constructor(code: ErrorCode, message: string, details?: any, context?: string);
    toString(): string;
    toJSON(): {
        name: string;
        code: ErrorCode;
        message: string;
        context: string | undefined;
        details: any;
        timestamp: string;
        stack: string | undefined;
    };
}
export declare class ErrorHandler {
    private static instance;
    private errorLog;
    static getInstance(): ErrorHandler;
    handleError(error: any, context?: string): ZephyrusTestError;
    private convertToTestError;
    createError(code: ErrorCode, message: string, details?: any, context?: string): ZephyrusTestError;
    getErrorLog(): ZephyrusTestError[];
    clearErrorLog(): void;
    getErrorsByCode(code: ErrorCode): ZephyrusTestError[];
    hasErrors(): boolean;
    hasCriticalErrors(): boolean;
    generateErrorSummary(): string;
}
export declare function handleAsyncError<T>(promise: Promise<T>, context?: string): Promise<T>;
export declare function wrapWithErrorHandling<T extends any[], R>(fn: (...args: T) => R, context?: string): (...args: T) => R;
export declare function wrapAsyncWithErrorHandling<T extends any[], R>(fn: (...args: T) => Promise<R>, context?: string): (...args: T) => Promise<R>;
export declare function validateRequired<T>(value: T | null | undefined, name: string, context?: string): T;
export declare function validatePositive(value: number, name: string, context?: string): number;
export declare function validateArray<T>(value: any, name: string, context?: string): T[];
export declare function validateString(value: any, name: string, context?: string): string;
export declare function validateNumber(value: any, name: string, context?: string): number;
export declare function retryWithErrorHandling<T>(operation: () => Promise<T>, maxRetries?: number, delay?: number, context?: string): Promise<T>;
//# sourceMappingURL=error-handler.d.ts.map