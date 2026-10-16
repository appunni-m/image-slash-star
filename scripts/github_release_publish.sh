
# Keep this inline logic mirrored in scripts/github_release_publish.sh for tests.
set -euo pipefail

tag="$GITHUB_REF_NAME"
version="${tag#v}"
repo="$GH_REPO"
bundle_dir=registry-bundle
crate_name="image-slash-star-${version}.crate"
expected_title="image-slash-star ${version}"
crate_path="$bundle_dir/$crate_name"
checksums_path="$bundle_dir/SHA256SUMS"
notes_path="$bundle_dir/release-notes.md"

[[ "$VERIFIED_TAG_COMMIT" == "$GITHUB_SHA" ]] || {
  echo "The pre-attestation tag check did not resolve to GITHUB_SHA." >&2
  exit 1
}
[[ -s "$crate_path" && -s "$checksums_path" && -s "$notes_path" ]] || {
  echo "The verified registry bundle is incomplete." >&2
  exit 1
}
(cd "$bundle_dir" && sha256sum --check SHA256SUMS)

crate_digest="$(sha256sum "$crate_path" | cut -d ' ' -f 1)"
checksums_digest="$(sha256sum "$checksums_path" | cut -d ' ' -f 1)"
expected_body="$RUNNER_TEMP/github-release-notes-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}.md"
cp "$notes_path" "$expected_body"
printf '\n<!-- image-slash-star-release tag=%s commit=%s crate-sha256=%s checksums-sha256=%s -->\n' \
  "$tag" "$GITHUB_SHA" "$crate_digest" "$checksums_digest" \
  >> "$expected_body"

release_endpoint="repos/$repo/releases/tags/$tag"
release_file="$RUNNER_TEMP/github-release-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}.json"
release_error="$RUNNER_TEMP/github-release-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}.error"
release_state=

fetch_release() {
  if gh api "$release_endpoint" > "$release_file" 2> "$release_error"; then
    release_state=present
  elif grep -Eq 'HTTP 404([^0-9]|$)' "$release_error"; then
    release_state=absent
  else
    echo "Unable to determine the GitHub release state; failing closed." >&2
    cat "$release_error" >&2
    return 1
  fi
}

validate_release_metadata() {
  jq -e \
    --arg tag "$tag" \
    --arg title "$expected_title" \
    --arg crate "$crate_name" \
    --rawfile body "$expected_body" \
    'type == "object"
     and .tag_name == $tag
     and .name == $title
     and .body == $body
     and .prerelease == true
     and (.draft | type == "boolean")
     and (.assets | type == "array"
          and all(.[]; .state == "uploaded"
                  and (.name == $crate or .name == "SHA256SUMS")))
     and (if .draft == false
          then ([.assets[].name] | sort) == ([$crate, "SHA256SUMS"] | sort)
          else true
          end)' \
    "$release_file" >/dev/null
}

has_asset() {
  jq -e --arg name "$1" \
    '.assets | any(.[]; .name == $name)' \
    "$release_file" >/dev/null
}

verify_present_assets() {
  local download_dir asset
  local patterns=()
  download_dir="$(mktemp -d "$RUNNER_TEMP/github-release-assets.XXXXXX")"
  for asset in "$crate_name" SHA256SUMS; do
    if has_asset "$asset"; then
      patterns+=(--pattern "$asset")
    fi
  done
  if ((${#patterns[@]} > 0)); then
    gh release download "$tag" --repo "$repo" \
      --dir "$download_dir" "${patterns[@]}"
    for asset in "$crate_name" SHA256SUMS; do
      if has_asset "$asset"; then
        if [[ ! -f "$download_dir/$asset" ]] \
          || ! cmp -s -- "$bundle_dir/$asset" "$download_dir/$asset"; then
          echo "GitHub release asset '$asset' does not match the verified registry bundle." >&2
          rm -rf -- "$download_dir"
          return 1
        fi
      fi
    done
  fi
  rm -rf -- "$download_dir"
}

verify_tag_commit() {
  local tag_ref tag_object tag_object_sha tag_commit
  if ! tag_ref="$(gh api "repos/$repo/git/ref/tags/$tag")"; then
    echo "Unable to revalidate the remote annotated tag before publishing." >&2
    return 1
  fi
  [[ "$(jq -r '.object.type // empty' <<< "$tag_ref")" == tag ]] || {
    echo "The remote release tag is no longer annotated." >&2
    return 1
  }
  tag_object_sha="$(jq -r '.object.sha // empty' <<< "$tag_ref")"
  [[ "$tag_object_sha" =~ ^[0-9a-f]{40}$ ]] || {
    echo "The remote annotated tag object has an invalid SHA." >&2
    return 1
  }
  if ! tag_object="$(gh api "repos/$repo/git/tags/$tag_object_sha")"; then
    echo "Unable to resolve the remote annotated tag object." >&2
    return 1
  fi
  [[ "$(jq -r '.object.type // empty' <<< "$tag_object")" == commit ]] || {
    echo "The remote annotated tag does not point directly to a commit." >&2
    return 1
  }
  tag_commit="$(jq -r '.object.sha // empty' <<< "$tag_object")"
  [[ "$tag_commit" == "$GITHUB_SHA" && "$tag_commit" == "$VERIFIED_TAG_COMMIT" ]] || {
    echo "The remote release tag no longer resolves to the verified commit." >&2
    return 1
  }
}

fetch_release
if [[ "$release_state" == absent ]]; then
  create_log="$RUNNER_TEMP/github-release-create-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}.log"
  if ! gh release create "$tag" --repo "$repo" \
    --verify-tag --draft --prerelease --title "$expected_title" \
    --notes-file "$expected_body" > "$create_log" 2>&1; then
    echo "Draft creation returned an error; checking whether GitHub accepted it." >&2
  fi
  fetch_release
  if [[ "$release_state" == absent ]]; then
    cat "$create_log" >&2
    echo "GitHub did not create a draft release for $tag." >&2
    exit 1
  fi
fi

if ! validate_release_metadata; then
  echo "An existing release has a different tag, commit marker, title, notes, type, or unexpected assets; no changes were made." >&2
  exit 1
fi
if ! verify_present_assets; then
  echo "An existing release asset is not the verified registry artifact; no changes were made." >&2
  exit 1
fi

if [[ "$(jq -r '.draft' "$release_file")" == false ]]; then
  echo "Verified the already-published immutable GitHub release for $tag."
  exit 0
fi

missing_assets=()
for asset in "$crate_name" SHA256SUMS; do
  if ! has_asset "$asset"; then
    missing_assets+=("$bundle_dir/$asset")
  fi
done
if ((${#missing_assets[@]} > 0)); then
  if ! gh release upload "$tag" --repo "$repo" "${missing_assets[@]}"; then
    echo "Asset upload returned an error; checking which verified assets GitHub accepted." >&2
  fi
  fetch_release
  if ! validate_release_metadata || ! verify_present_assets; then
    echo "The draft release changed unexpectedly during asset upload; no existing asset was overwritten." >&2
    exit 1
  fi
fi

if ! jq -e --arg crate "$crate_name" \
  '([.assets[].name] | sort) == ([$crate, "SHA256SUMS"] | sort)
   and all(.assets[]; .state == "uploaded")' \
  "$release_file" >/dev/null; then
  echo "The matching draft is incomplete; rerun this job to retry missing asset uploads." >&2
  exit 1
fi
if ! verify_present_assets; then
  echo "The completed draft assets differ from the verified registry bundle." >&2
  exit 1
fi

verify_tag_commit
if ! gh release edit "$tag" --repo "$repo" --draft=false; then
  echo "Draft publication returned an error; checking whether GitHub published it." >&2
fi
fetch_release
if [[ "$release_state" != present ]] || ! validate_release_metadata; then
  echo "Unable to verify the GitHub release after publication; no asset was overwritten." >&2
  exit 1
fi
if [[ "$(jq -r '.draft' "$release_file")" != false ]]; then
  echo "The verified release remains a draft; rerun this job to retry publishing it." >&2
  exit 1
fi
if ! jq -e --arg crate "$crate_name" \
  '([.assets[].name] | sort) == ([$crate, "SHA256SUMS"] | sort)
   and all(.assets[]; .state == "uploaded")' \
  "$release_file" >/dev/null || ! verify_present_assets; then
  echo "The published GitHub release does not match the verified registry bundle." >&2
  exit 1
fi
echo "Published and verified the immutable GitHub release for $tag."
