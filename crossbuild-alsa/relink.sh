find . -lname '/*' | while read -r link; do
  abs_target=$(readlink "$link")
  ln -fs "$(pwd)$abs_target" "$link"
done

symlinks -rc .
