import { randomUUID } from "node:crypto";

export function requestId(request: Request): string {
  const supplied = request.headers.get("x-request-id")?.trim();
  return supplied && /^[A-Za-z0-9._:-]{8,128}$/.test(supplied) ? supplied : randomUUID();
}

export function jsonError(
  request: Request,
  status: number,
  code: string,
  message: string,
  details?: Record<string, unknown>,
): Response {
  const id = requestId(request);
  return Response.json(
    { error: message, code, requestId: id, ...(details ? { details } : {}) },
    {
      status,
      headers: {
        "Cache-Control": "no-store",
        "X-Request-Id": id,
      },
    },
  );
}

export function jsonOk<T>(request: Request, body: T, init: ResponseInit = {}): Response {
  const id = requestId(request);
  const headers = new Headers(init.headers);
  headers.set("X-Request-Id", id);
  return Response.json(body, { ...init, headers });
}

export async function requestJson(request: Request): Promise<unknown> {
  const contentLength = Number.parseInt(request.headers.get("content-length") ?? "0", 10);
  if (Number.isFinite(contentLength) && contentLength > 1_048_576) throw new Error("FTEP_REQUEST_TOO_LARGE");
  return await request.json();
}
