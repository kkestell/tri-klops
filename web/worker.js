import init, { generate } from "./triklops.js";

let stopped = false;

self.onmessage = async ({ data }) => {
  if (data === "stop") {
    stopped = true;
    return;
  }

  try {
    await init();
    const result = await generate(data, async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
      return stopped;
    });
    self.postMessage(result);
  } catch (error) {
    self.postMessage({ type: "error", message: String(error) });
  }
};
