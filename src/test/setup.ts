import { configure } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

// App.test renders the whole shell; with every worker busy a findBy can
// need more than the default 1 s and fail intermittently.
configure({ asyncUtilTimeout: 3_000 });
