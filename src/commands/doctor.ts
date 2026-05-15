import { render } from "ink";
import React from "react";
import { loadConfig } from "../config/loader";
import { Validating } from "../tui/screens/Validating";

export async function doctorCommand(cwd: string, revalidate: boolean): Promise<void> {
  const loaded = await loadConfig(cwd);
  await new Promise<void>((res, rej) => {
    const app = render(
      React.createElement(Validating, {
        validators: loaded.config.validators,
        repoHash: loaded.repoHash,
        configHash: loaded.configHash,
        revalidate,
        onDone: () => {
          app.unmount();
          res();
        },
        onError: (err: Error) => {
          app.unmount();
          rej(err);
        },
      }),
    );
  });
}
