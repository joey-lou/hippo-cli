// Pi session adapter. Injects a short Hippo digest. Fails open.
// Install: adapters/pi/install.sh
// Recall and save stay on the `hippo` command; see skills/use-memory/SKILL.md.

export default function (pi) {
  pi.on("session_start", async () => {
    try {
      await pi.exec("hippo", ["reindex", "--changed"], { timeout: 20000 });
      const digest = await pi.exec("hippo", ["digest"], { timeout: 10000 });
      const text = (digest.stdout || "").trim();
      if (!text) return;
      pi.sendMessage(
        {
          customType: "hippo-digest",
          content: [
            "Hippo memory digest. This is a count and topic list, not the memories.",
            "Recall with `hippo query \"<text>\" --json` and read only the top files.",
            "Save durable facts with `hippo add` or `hippo update` after a query.",
            "",
            text,
          ].join("\n"),
          display: false,
        },
        { triggerTurn: false, deliverAs: "nextTurn" },
      );
    } catch {
      // A missing hippo binary must not break the session.
    }
  });
}
