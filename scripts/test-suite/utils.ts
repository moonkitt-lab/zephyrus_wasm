import * as path from "node:path";
import { exec } from "child_process";
import { promises as fs } from "fs";

export function requiredEnvVar(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Environment variable ${name} is required but not set.`);
  }
  return value;
}

export function artifact(name: string): string {
  return `${__dirname}/../../artifacts/${name}.wasm`;
}

export async function readContractFileBytes(
  filePath: string,
): Promise<Uint8Array> {
  try {
    await fs.access(filePath);
    const contents = await fs.readFile(filePath);
    return new Uint8Array(contents);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      throw new Error(`Contract file ${filePath} does not exist`);
    }

    throw error;
  }
}

export const getFileNameWithoutExtension = (filePath: string) =>
  path.basename(filePath, path.extname(filePath));

export const snakeCaseToKebabCase = (str: string) => str.replace(/_/g, "-");

export async function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function waitFor(
  fn: () => Promise<boolean>,
  timeout: number = 10000,
  interval: number = 600,
): Promise<void> {
  const start = Date.now();
  while (true) {
    if (await fn()) {
      break;
    }
    if (Date.now() - start > timeout) {
      throw new Error("Waiting for condition timed out");
    }
    await sleep(interval);
  }
}

export function runCommand(command: string, unsafe = false): Promise<string> {
  return new Promise((resolve, reject) => {
    exec(command, (error, stdout, stderr) => {
      if (unsafe) {
        resolve(stdout);
        return;
      }
      if (error) {
        return reject(error);
      }
      if (stderr) {
        return reject(stderr);
      }
      resolve(stdout);
    });
  });
}

export async function isContainerPaused(
  containerName: string,
): Promise<boolean> {
  try {
    await sleep(1000);
    const output = await runCommand(
      `docker inspect -f '{{.State.Paused}}' ${containerName}`,
    );
    return output.trim() === "true";
  } catch (error) {
    console.error(`Error checking container status: ${error}`);
    return false;
  }
}
