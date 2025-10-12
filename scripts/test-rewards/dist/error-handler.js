"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.ErrorHandler = exports.ZephyrusTestError = exports.ErrorCode = void 0;
exports.handleAsyncError = handleAsyncError;
exports.wrapWithErrorHandling = wrapWithErrorHandling;
exports.wrapAsyncWithErrorHandling = wrapAsyncWithErrorHandling;
exports.validateRequired = validateRequired;
exports.validatePositive = validatePositive;
exports.validateArray = validateArray;
exports.validateString = validateString;
exports.validateNumber = validateNumber;
exports.retryWithErrorHandling = retryWithErrorHandling;
var ErrorCode;
(function (ErrorCode) {
    // Configuration Errors
    ErrorCode["CONFIG_INVALID"] = "CONFIG_INVALID";
    ErrorCode["CONFIG_MISSING"] = "CONFIG_MISSING";
    // Wallet Errors
    ErrorCode["WALLET_CREATE_FAILED"] = "WALLET_CREATE_FAILED";
    ErrorCode["WALLET_FUNDING_FAILED"] = "WALLET_FUNDING_FAILED";
    ErrorCode["WALLET_INSUFFICIENT_FUNDS"] = "WALLET_INSUFFICIENT_FUNDS";
    // Contract Errors
    ErrorCode["CONTRACT_NOT_FOUND"] = "CONTRACT_NOT_FOUND";
    ErrorCode["CONTRACT_CALL_FAILED"] = "CONTRACT_CALL_FAILED";
    ErrorCode["CONTRACT_QUERY_FAILED"] = "CONTRACT_QUERY_FAILED";
    ErrorCode["CONTRACT_INSTANTIATE_FAILED"] = "CONTRACT_INSTANTIATE_FAILED";
    // Transaction Errors
    ErrorCode["TX_FAILED"] = "TX_FAILED";
    ErrorCode["TX_TIMEOUT"] = "TX_TIMEOUT";
    ErrorCode["TX_INSUFFICIENT_GAS"] = "TX_INSUFFICIENT_GAS";
    // Scenario Errors
    ErrorCode["SCENARIO_INVALID"] = "SCENARIO_INVALID";
    ErrorCode["SCENARIO_EXECUTION_FAILED"] = "SCENARIO_EXECUTION_FAILED";
    // Validation Errors
    ErrorCode["REWARDS_MISMATCH"] = "REWARDS_MISMATCH";
    ErrorCode["VALIDATION_FAILED"] = "VALIDATION_FAILED";
    // Network Errors
    ErrorCode["NETWORK_UNREACHABLE"] = "NETWORK_UNREACHABLE";
    ErrorCode["RPC_ERROR"] = "RPC_ERROR";
    // File System Errors
    ErrorCode["FILE_NOT_FOUND"] = "FILE_NOT_FOUND";
    ErrorCode["FILE_READ_ERROR"] = "FILE_READ_ERROR";
    ErrorCode["FILE_WRITE_ERROR"] = "FILE_WRITE_ERROR";
    // Generic Errors
    ErrorCode["UNKNOWN_ERROR"] = "UNKNOWN_ERROR";
    ErrorCode["TIMEOUT_ERROR"] = "TIMEOUT_ERROR";
})(ErrorCode || (exports.ErrorCode = ErrorCode = {}));
class ZephyrusTestError extends Error {
    constructor(code, message, details, context) {
        super(message);
        this.name = "ZephyrusTestError";
        this.code = code;
        this.details = details;
        this.timestamp = new Date();
        this.context = context;
    }
    toString() {
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
exports.ZephyrusTestError = ZephyrusTestError;
class ErrorHandler {
    constructor() {
        this.errorLog = [];
    }
    static getInstance() {
        if (!ErrorHandler.instance) {
            ErrorHandler.instance = new ErrorHandler();
        }
        return ErrorHandler.instance;
    }
    handleError(error, context) {
        let testError;
        if (error instanceof ZephyrusTestError) {
            testError = error;
        }
        else {
            testError = this.convertToTestError(error, context);
        }
        this.errorLog.push(testError);
        return testError;
    }
    convertToTestError(error, context) {
        const message = error.message || String(error);
        let code = ErrorCode.UNKNOWN_ERROR;
        // Categorize error based on message content
        if (message.includes("contract") && message.includes("not found")) {
            code = ErrorCode.CONTRACT_NOT_FOUND;
        }
        else if (message.includes("insufficient funds") || message.includes("insufficient balance")) {
            code = ErrorCode.WALLET_INSUFFICIENT_FUNDS;
        }
        else if (message.includes("gas")) {
            code = ErrorCode.TX_INSUFFICIENT_GAS;
        }
        else if (message.includes("timeout")) {
            code = ErrorCode.TX_TIMEOUT;
        }
        else if (message.includes("network") || message.includes("connection")) {
            code = ErrorCode.NETWORK_UNREACHABLE;
        }
        else if (message.includes("rpc") || message.includes("RPC")) {
            code = ErrorCode.RPC_ERROR;
        }
        else if (message.includes("file") && message.includes("not found")) {
            code = ErrorCode.FILE_NOT_FOUND;
        }
        else if (message.includes("scenario") && message.includes("invalid")) {
            code = ErrorCode.SCENARIO_INVALID;
        }
        else if (message.includes("transaction failed")) {
            code = ErrorCode.TX_FAILED;
        }
        return new ZephyrusTestError(code, message, error, context);
    }
    createError(code, message, details, context) {
        return new ZephyrusTestError(code, message, details, context);
    }
    getErrorLog() {
        return [...this.errorLog];
    }
    clearErrorLog() {
        this.errorLog = [];
    }
    getErrorsByCode(code) {
        return this.errorLog.filter(error => error.code === code);
    }
    hasErrors() {
        return this.errorLog.length > 0;
    }
    hasCriticalErrors() {
        const criticalCodes = [
            ErrorCode.CONTRACT_NOT_FOUND,
            ErrorCode.NETWORK_UNREACHABLE,
            ErrorCode.WALLET_CREATE_FAILED,
            ErrorCode.CONFIG_INVALID
        ];
        return this.errorLog.some(error => criticalCodes.includes(error.code));
    }
    generateErrorSummary() {
        if (this.errorLog.length === 0) {
            return "No errors recorded";
        }
        const errorsByCode = new Map();
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
exports.ErrorHandler = ErrorHandler;
// Utility functions for common error scenarios
function handleAsyncError(promise, context) {
    return promise.catch(error => {
        const testError = ErrorHandler.getInstance().handleError(error, context);
        throw testError;
    });
}
function wrapWithErrorHandling(fn, context) {
    return (...args) => {
        try {
            return fn(...args);
        }
        catch (error) {
            const testError = ErrorHandler.getInstance().handleError(error, context);
            throw testError;
        }
    };
}
function wrapAsyncWithErrorHandling(fn, context) {
    return async (...args) => {
        try {
            return await fn(...args);
        }
        catch (error) {
            const testError = ErrorHandler.getInstance().handleError(error, context);
            throw testError;
        }
    };
}
// Validation helper functions
function validateRequired(value, name, context) {
    if (value === null || value === undefined) {
        throw ErrorHandler.getInstance().createError(ErrorCode.CONFIG_MISSING, `Required value '${name}' is missing`, { name, value }, context);
    }
    return value;
}
function validatePositive(value, name, context) {
    if (value <= 0) {
        throw ErrorHandler.getInstance().createError(ErrorCode.CONFIG_INVALID, `Value '${name}' must be positive, got: ${value}`, { name, value }, context);
    }
    return value;
}
function validateArray(value, name, context) {
    if (!Array.isArray(value)) {
        throw ErrorHandler.getInstance().createError(ErrorCode.CONFIG_INVALID, `Value '${name}' must be an array, got: ${typeof value}`, { name, value, type: typeof value }, context);
    }
    return value;
}
function validateString(value, name, context) {
    if (typeof value !== "string") {
        throw ErrorHandler.getInstance().createError(ErrorCode.CONFIG_INVALID, `Value '${name}' must be a string, got: ${typeof value}`, { name, value, type: typeof value }, context);
    }
    return value;
}
function validateNumber(value, name, context) {
    if (typeof value !== "number" || isNaN(value)) {
        throw ErrorHandler.getInstance().createError(ErrorCode.CONFIG_INVALID, `Value '${name}' must be a valid number, got: ${value}`, { name, value, type: typeof value }, context);
    }
    return value;
}
// Retry mechanism with error handling
async function retryWithErrorHandling(operation, maxRetries = 3, delay = 1000, context) {
    let lastError;
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            return await operation();
        }
        catch (error) {
            lastError = error;
            if (attempt === maxRetries) {
                const testError = ErrorHandler.getInstance().handleError(lastError, `${context} (after ${maxRetries} retries)`);
                throw testError;
            }
            // Wait before retrying
            await new Promise(resolve => setTimeout(resolve, delay * attempt));
        }
    }
    // This should never be reached, but TypeScript requires it
    throw lastError;
}
//# sourceMappingURL=error-handler.js.map