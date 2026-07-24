<!--
Domain: Sidebar skeleton
Owns: Loading skeleton presentation for the reserved sidebar accordion.
Excludes: Sidebar widget internals, loading policy, and layout state.
Zone: Layout edge-lane presentation component with no script dependencies.
-->
<aside
  class="flex h-full w-full min-h-0 min-w-0 flex-col gap-2 rounded-2xl bg-[linear-gradient(135deg,#ffffff_0%,#f2f8ec_46%,#edf6fa_100%)] p-3"
  aria-hidden="true"
>
  <div
    class="grid animate-pulse overflow-hidden rounded-xl border border-(--mono-border)/30 bg-white/70"
  >
    <div
      class="flex h-11 items-center justify-center border-b border-(--mono-border)/20"
    >
      <div class="h-3 w-20 rounded-full bg-(--mono-border)/14"></div>
    </div>
    <div class="grid gap-2 p-3">
      <div class="h-14 rounded-xl bg-(--mono-border)/10"></div>
      <div class="h-14 rounded-xl bg-(--mono-border)/10"></div>
      <div class="h-10 rounded-xl bg-(--mono-border)/10"></div>
    </div>
  </div>
  <div
    class="flex h-11 animate-pulse items-center justify-center rounded-xl border border-(--mono-border)/30 bg-white/70"
  >
    <div class="h-3 w-20 rounded-full bg-(--mono-border)/14"></div>
  </div>
</aside>
