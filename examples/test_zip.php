<?php
$zip = new ZipArchive();
$filename = "/tmp/test.zip";

// Open the zip file for creation (ZipArchive::CREATE)
if ($zip->open($filename, ZipArchive::CREATE) !== TRUE) {
    exit("cannot open <$filename>\n");
}

file_put_contents('/tmp/data.txt', "This is some data in a file.\n");

// 1. Add a file from an existing path on the server
$zip->addFile('/tmp/data.txt', 'entryname.txt'); // source file path, name inside zip

// 2. Add a file by providing its content as a string
$zip->addFromString('testfilephp.txt', "#1 This is test file content.\n");

// All files have been added, so close the zip file to save it
$zip->close();
echo "Created $filename successfully.";