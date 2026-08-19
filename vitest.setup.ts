import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// React Testing Library does not auto-clean when `globals: true` is combined
// with a custom environment, so unmount between tests to stop state leaking.
afterEach(cleanup);
