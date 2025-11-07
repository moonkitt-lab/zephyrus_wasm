import { promisify } from "util";
import { exec } from "child_process";
import fs from "fs";
import path from "path";
import codegen from "@cosmwasm/ts-codegen";

const execAsync = promisify(exec);

async function getCargoPackages() {
  const { stdout } = await execAsync('find contracts -name "Cargo.toml"');
  return stdout.split("\n").filter(Boolean);
}

function extractPackageName(cargoTomlPath) {
  const content = fs.readFileSync(cargoTomlPath, "utf-8");
  const nameMatch = content.match(/name\s*=\s*"(.*?)"/);
  return nameMatch ? nameMatch[1] : null;
}

function hasSchemaBinary(cargoTomlPath) {
  const content = fs.readFileSync(cargoTomlPath, "utf-8");
  return /\[\[bin\]\]\s*name\s*=\s*".*schema.*"/.test(content);
}

function toPascalCase(str) {
  return str.replace(/(^\w|-\w)/g, clearAndUpper).replace(/-/g, "");
}

function clearAndUpper(text) {
  return text.replace(/-/, "").toUpperCase();
}

async function generateBindings() {
  const cargoTomlPaths = await getCargoPackages();

  for (const cargoTomlPath of cargoTomlPaths) {
    if (hasSchemaBinary(cargoTomlPath)) {
      const packageName = extractPackageName(cargoTomlPath);
      if (packageName) {
        const contractName = toPascalCase(packageName);
        const schemaDir = path.resolve(process.cwd(), "schema", packageName);
        const outPath = path.resolve(process.cwd(), "ts");

        console.log(
          `Generating TypeScript bindings for ${packageName} from ${schemaDir}...`
        );

        await codegen({
          contracts: [
            {
              name: contractName,
              dir: schemaDir,
            },
          ],
          outPath,
          options: {
            bundle: {
              bundleFile: "index.ts",
              scope: "contracts",
            },
            types: {
              enabled: true,
            },
            client: {
              enabled: true,
            },
            reactQuery: {
              enabled: false,
            },
            recoil: {
              enabled: false,
            },
            messageComposer: {
              enabled: true,
            },
            messageBuilder: {
              enabled: false,
            },
            useContractsHook: {
              enabled: false,
            },
          },
        });
      }
    }
  }
}

generateBindings().catch((error) => {
  console.error("Error generating bindings:", error);
});