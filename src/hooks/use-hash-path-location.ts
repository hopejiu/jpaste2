/**
 * wouter hash-location wrapper that strips the query-string part from the
 * hash before passing it to the router.  wouter v3's `matchRoute` feeds
 * the raw location into `regexparam` which is anchored with `$` (exact
 * match), so `/viewer/json?id=22` never matches the route `/viewer/json`.
 *
 * By splitting on `?` we give the router just the path portion, which
 * allows exact and parameterised routes to match correctly.  Query
 * parameters inside the hash are still accessible via
 * `new URLSearchParams(window.location.hash.split('?')[1])` in the
 * target component.
 */
import { useHashLocation } from 'wouter/use-hash-location';

type NavigateFn = (to: string, opts?: { replace?: boolean; state?: unknown }) => void;

export function useHashPathLocation(): [string, NavigateFn] {
  const [location, navigate] = useHashLocation() as [string, NavigateFn];
  // Strip query string — only the path part is used for route matching
  const pathOnly = location.split('?')[0];
  return [pathOnly, navigate];
}
