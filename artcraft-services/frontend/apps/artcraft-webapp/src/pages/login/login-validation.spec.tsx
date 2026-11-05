import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { UsersApi } from "@storyteller/api";
import * as session from "../../lib/session";
import Login from "./login";

// This unit covers the password form. Keep our Google button real, but do not
// initialize an unrelated external OAuth SDK or claim Google authentication.
vi.mock("@react-oauth/google", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@react-oauth/google")>()),
  GoogleLogin: () => null,
}));

beforeEach(() => {
  vi.spyOn(UsersApi.prototype, "Login").mockResolvedValue({
    success: false,
    errorMessage: "Server rejected test credentials",
  });
  vi.spyOn(session, "refreshSession").mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("password login required fields", () => {
  it.each([
    ["", "", "both empty"],
    [" \t ", "p", "whitespace-only username"],
    ["alice", "", "empty password"],
  ])("rejects %s/%s (%s) locally without calling the API", (username, password) => {
    showLogin();
    enter(username, password);
    fireEvent.click(screen.getByRole("button", { name: "Log in" }));
    expect(UsersApi.prototype.Login).not.toHaveBeenCalled();
    expect(session.refreshSession).not.toHaveBeenCalled();
    expect(screen.getByRole("alert").textContent).toBe("Enter your email or username and password.");
    expect((screen.getByRole("button", { name: "Log in" }) as HTMLButtonElement).disabled).toBe(false);
  });

  it.each([
    ["alice", "p"],
    [" alice ", " "],
  ])("preserves the original nonempty payload %s/%s without imposing a login password length", async (username, password) => {
    showLogin();
    enter(username, password);
    fireEvent.click(screen.getByRole("button", { name: "Log in" }));
    await screen.findByText("Server rejected test credentials");
    expect(UsersApi.prototype.Login).toHaveBeenCalledExactlyOnceWith({ usernameOrEmail: username, password });
    expect(session.refreshSession).not.toHaveBeenCalled();
  });

  it("allows a corrected form to submit after a local required-field error", async () => {
    showLogin();
    fireEvent.click(screen.getByRole("button", { name: "Log in" }));
    expect(UsersApi.prototype.Login).not.toHaveBeenCalled();
    enter("alice", "p");
    fireEvent.click(screen.getByRole("button", { name: "Log in" }));
    await screen.findByText("Server rejected test credentials");
    expect(UsersApi.prototype.Login).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Enter your email or username and password.")).toBeNull();
  });

  it("retains session refresh before the existing successful return-path navigation", async () => {
    vi.mocked(UsersApi.prototype.Login).mockResolvedValue({ success: true, data: {} });
    showLogin();
    enter("alice", "p");
    fireEvent.click(screen.getByRole("button", { name: "Log in" }));
    await screen.findByText("Existing return destination");
    expect(session.refreshSession).toHaveBeenCalledExactlyOnceWith(true);
    await waitFor(() => expect(UsersApi.prototype.Login).toHaveBeenCalledTimes(1));
  });
});

function showLogin() {
  return render(
    <MemoryRouter initialEntries={["/login?from=%2Flibrary"]}>
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="/library" element={<p>Existing return destination</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

function enter(username: string, password: string) {
  fireEvent.change(screen.getByPlaceholderText("you@example.com or username"), { target: { value: username } });
  fireEvent.change(screen.getByPlaceholderText("Min. 8 characters"), { target: { value: password } });
}
