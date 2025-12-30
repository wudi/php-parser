<?php

echo "before";
try {
    throw new Exception("test");
} finally {
    echo " finally";
}
echo " after";
