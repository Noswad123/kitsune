import { beforeEach, expect, mock, test } from "bun:test";

const requests: unknown[] = [];
let importCounter = 0;

mock.module("node:net", () => ({
  default: {
    createConnection(_path: string, onConnect: () => void) {
      const handlers = new Map<string, () => void>();
      const client = {
        write(input: string) {
          requests.push(JSON.parse(input.trim()));
          queueMicrotask(() => client.emit("data"));
        },
        setTimeout() {},
        on(event: string, handler: () => void) {
          handlers.set(event, handler);
        },
        destroy() {},
        emit(event: string) {
          handlers.get(event)?.();
        },
      };
      queueMicrotask(onConnect);
      return client;
    },
  },
}));

beforeEach(() => {
  requests.length = 0;
  process.env.KITSUNE_ENV = "1";
  process.env.KITSUNE_SOCKET_PATH = "test.sock";
  process.env.KITSUNE_PANE_ID = "test:p1";
});

async function loadPlugin() {
  importCounter += 1;
  const { KitsuneAgentStatePlugin } = await import(`./kitsune-agent-state.js?test=${importCounter}`);
  return KitsuneAgentStatePlugin();
}

test("reports buddy identity", async () => {
  const plugin = await loadPlugin();

  await plugin.event({
    event: {
      type: "session.status",
      properties: { sessionID: "root-session", status: { type: "idle" } },
    },
  });

  expect(requests.map(requestParam("source"))).toEqual(["kitsune:buddy"]);
  expect(requests.map(requestParam("agent"))).toEqual(["buddy"]);
  expect(requests.map(requestParam("agent_session_id"))).toEqual(["root-session"]);
});

test("releases the root session when buddy deletes it", async () => {
  const plugin = await loadPlugin();

  await plugin.event({
    event: {
      type: "session.status",
      properties: { sessionID: "root-session", status: { type: "busy" } },
    },
  });
  await plugin.event({
    event: { type: "session.deleted", properties: { sessionID: "root-session" } },
  });

  expect(requests.map(requestMethod)).toEqual([
    "pane.report_agent",
    "pane.release_agent",
  ]);
  expect(requests.map(requestParam("agent_session_id"))).toEqual([
    "root-session",
    "root-session",
  ]);
});

test("does not release root authority for child session deletes", async () => {
  const plugin = await loadPlugin();

  await plugin.event({
    event: {
      type: "session.created",
      properties: {
        sessionID: "child-session",
        info: { id: "child-session", parentID: "root-session" },
      },
    },
  });
  await plugin.event({
    event: { type: "session.deleted", properties: { sessionID: "child-session" } },
  });

  expect(requests).toEqual([]);
});

function requestMethod(request: unknown): unknown {
  return typeof request === "object" && request !== null && "method" in request
    ? request.method
    : undefined;
}

function requestParam(name: string) {
  return (request: unknown): unknown => {
    if (typeof request !== "object" || request === null || !("params" in request)) return undefined;
    if (typeof request.params !== "object" || request.params === null || !(name in request.params)) return undefined;
    return request.params[name];
  };
}
