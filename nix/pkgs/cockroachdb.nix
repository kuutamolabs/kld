{ cockroachdb }:
cockroachdb.overrideAttrs (old: {
  # avoid having to deal with unfree license check overlays...
  meta = old.meta // { license = [ ]; };
})
